// Many things will be used in the future
#![allow(unused)]

//! Module holding safe wrappers over winapi functions

use std::{ffi::c_void, fmt::Error, io, mem, ops::Deref, ptr};
use windows::{
    core::{GUID, HRESULT, PCWSTR},
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            SetupDiBuildDriverInfoList, SetupDiCallClassInstaller, SetupDiClassNameFromGuidW,
            SetupDiCreateDeviceInfoList, SetupDiCreateDeviceInfoW, SetupDiDestroyDeviceInfoList,
            SetupDiDestroyDriverInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDriverInfoW, SetupDiGetClassDevsW,
            SetupDiGetDeviceRegistryPropertyW, SetupDiGetDriverInfoDetailW, SetupDiOpenDevRegKey,
            SetupDiSetClassInstallParamsW, SetupDiSetDeviceRegistryPropertyW, SetupDiSetSelectedDevice,
            SetupDiSetSelectedDriverW, DI_FUNCTION, HDEVINFO, MAX_CLASS_NAME_LEN, SETUP_DI_DEVICE_CREATION_FLAGS,
            SETUP_DI_DRIVER_TYPE, SETUP_DI_GET_CLASS_DEVS_FLAGS, SETUP_DI_REGISTRY_PROPERTY, SP_CLASSINSTALL_HEADER,
            SP_DEVINFO_DATA, SP_DRVINFO_DATA_V2_W, SP_DRVINFO_DETAIL_DATA_W,
        },
        Foundation::{
            CloseHandle, GetLastError, BOOL, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_PENDING, ERROR_NO_MORE_ITEMS, FALSE,
            FILETIME, HANDLE, HWND, TRUE, WAIT_EVENT, WAIT_OBJECT_0, WAIT_TIMEOUT, WIN32_ERROR,
        },
        NetworkManagement::{
            IpHelper::{
                ConvertInterfaceAliasToLuid, ConvertInterfaceLuidToAlias, ConvertInterfaceLuidToGuid,
                ConvertInterfaceLuidToIndex,
            },
            Ndis::NET_LUID_LH,
        },
        Storage::FileSystem::{
            CreateFileW, ReadFile, WriteFile, FILE_CREATION_DISPOSITION, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE,
        },
        System::{
            Com::StringFromGUID2,
            Registry::{RegNotifyChangeKeyValue, HKEY, REG_NOTIFY_FILTER},
            Threading::{CreateEventW, WaitForSingleObject},
            IO::{DeviceIoControl, GetOverlappedResult, OVERLAPPED},
        },
    },
};

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
#[derive(Clone, Copy)]
/// Custom type to handle variable size SP_DRVINFO_DETAIL_DATA_W
pub struct SP_DRVINFO_DETAIL_DATA_W2 {
    pub cbSize: u32,
    pub InfDate: FILETIME,
    pub CompatIDsOffset: u32,
    pub CompatIDsLength: u32,
    pub Reserved: usize,
    pub SectionName: [u16; 256],
    pub InfFileName: [u16; 260],
    pub DrvDescription: [u16; 256],
    pub HardwareID: [u16; 512],
}

pub fn string_from_guid(guid: &GUID) -> io::Result<String> {
    unsafe {
        let mut string = vec![0; 64];
        if StringFromGUID2(guid, &mut string) == 0 {
            return Err(io::Error::last_os_error());
        }

        let string = PCWSTR(string.as_ptr())
            .to_string()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        Ok(string)
    }
}

pub fn alias_to_luid(alias: &str) -> io::Result<NET_LUID_LH> {
    let alias = alias.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut luid = unsafe { mem::zeroed() };
    unsafe {
        ConvertInterfaceAliasToLuid(PCWSTR(alias.as_ptr()), &mut luid).ok()?;
    }
    Ok(luid)
}

pub fn luid_to_index(luid: &NET_LUID_LH) -> io::Result<u32> {
    let mut index = 0;
    unsafe {
        ConvertInterfaceLuidToIndex(luid, &mut index).ok()?;
    }
    Ok(index)
}

pub fn luid_to_guid(luid: &NET_LUID_LH) -> io::Result<GUID> {
    let mut guid = unsafe { mem::zeroed() };
    unsafe {
        ConvertInterfaceLuidToGuid(luid, &mut guid).ok()?;
    }
    Ok(guid)
}

pub fn luid_to_alias(luid: &NET_LUID_LH) -> io::Result<String> {
    unsafe {
        // IF_MAX_STRING_SIZE + 1
        let mut alias = vec![0; 257];
        ConvertInterfaceLuidToAlias(luid, &mut alias).ok()?;

        let alias = PCWSTR(alias.as_ptr())
            .to_string()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        Ok(alias)
    }
}

pub fn close_handle(handle: HANDLE) -> io::Result<()> {
    unsafe {
        CloseHandle(handle);
    }
    Ok(())
}

pub fn create_file(
    file_name: &str,
    desired_access: u32,
    share_mode: FILE_SHARE_MODE,
    creation_disposition: FILE_CREATION_DISPOSITION,
    flags_and_attributes: FILE_FLAGS_AND_ATTRIBUTES,
) -> io::Result<HANDLE> {
    let file_name = file_name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(file_name.as_ptr()),
            desired_access,
            share_mode,
            None,
            creation_disposition,
            flags_and_attributes,
            None,
        )?
    };
    Ok(handle)
}

pub fn read_file(handle: HANDLE, buffer: &mut [u8]) -> io::Result<usize> {
    let mut ret = 0;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = unsafe { CreateEventW(None, true, false, None) }?;
    if let Err(error) = unsafe { ReadFile(handle, Some(buffer), Some(&mut ret), Some(&mut overlapped)) } {
        if error != ERROR_IO_PENDING.into() {
            return Err(error.into());
        }
        unsafe { GetOverlappedResult(handle, &mut overlapped, &mut ret, true) }?;
    }
    Ok(ret as _)
}

pub fn write_file(handle: HANDLE, buffer: &[u8]) -> io::Result<usize> {
    let mut ret = 0;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = unsafe { CreateEventW(None, true, false, None) }?;
    if let Err(error) = unsafe { WriteFile(handle, Some(buffer), Some(&mut ret), Some(&mut overlapped)) } {
        if error != ERROR_IO_PENDING.into() {
            return Err(error.into());
        }
        unsafe { GetOverlappedResult(handle, &mut overlapped, &mut ret, true) }?;
    }
    Ok(ret as _)
}

pub fn create_device_info_list(guid: &GUID) -> io::Result<HDEVINFO> {
    let devinfo = unsafe { SetupDiCreateDeviceInfoList(Some(guid), HWND::default())? };
    Ok(devinfo)
}

pub fn get_class_devs(guid: &GUID, flags: SETUP_DI_GET_CLASS_DEVS_FLAGS) -> io::Result<HDEVINFO> {
    let devinfo = unsafe { SetupDiGetClassDevsW(Some(guid), PCWSTR::null(), HWND::default(), flags)? };
    Ok(devinfo)
}

pub fn destroy_device_info_list(devinfo: HDEVINFO) -> io::Result<()> {
    unsafe {
        SetupDiDestroyDeviceInfoList(devinfo)?;
    }
    Ok(())
}

pub fn class_name_from_guid(guid: &GUID) -> io::Result<String> {
    unsafe {
        let mut class_name = vec![0; MAX_CLASS_NAME_LEN as usize];
        SetupDiClassNameFromGuidW(guid, &mut class_name, None)?;
        let class_name = PCWSTR(class_name.as_ptr())
            .to_string()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(class_name)
    }
}

pub fn create_device_info(
    devinfo: HDEVINFO,
    device_name: &str,
    guid: &GUID,
    device_description: &str,
    creation_flags: SETUP_DI_DEVICE_CREATION_FLAGS,
) -> io::Result<SP_DEVINFO_DATA> {
    let mut devinfo_data: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
    devinfo_data.cbSize = mem::size_of_val(&devinfo_data) as _;
    unsafe {
        let device_name = device_name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let device_description = device_description.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        SetupDiCreateDeviceInfoW(
            devinfo,
            PCWSTR(device_name.as_ptr()),
            guid,
            PCWSTR(device_description.as_ptr()),
            HWND::default(),
            creation_flags,
            Some(&mut devinfo_data),
        )?;
    }
    Ok(devinfo_data)
}

pub fn set_selected_device(devinfo: HDEVINFO, devinfo_data: &SP_DEVINFO_DATA) -> io::Result<()> {
    unsafe {
        SetupDiSetSelectedDevice(devinfo, devinfo_data)?;
    }
    Ok(())
}

pub fn set_device_registry_property(
    devinfo: HDEVINFO,
    devinfo_data: &mut SP_DEVINFO_DATA,
    property: SETUP_DI_REGISTRY_PROPERTY,
    value: Option<&str>,
) -> io::Result<()> {
    unsafe {
        // convert string from utf8 to utf16 null-terminated string and then force it to be little endian bytes
        let value = value.map(|v| {
            v.encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>()
                .iter()
                .flat_map(|&x| x.to_le_bytes().to_vec())
                .collect::<Vec<u8>>()
        });
        SetupDiSetDeviceRegistryPropertyW(devinfo, &mut *devinfo_data, property, value.as_deref())?;
    }
    Ok(())
}

pub fn get_device_registry_property(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    property: SETUP_DI_REGISTRY_PROPERTY,
) -> io::Result<String> {
    unsafe {
        let mut requiredsize = 0;

        let r = SetupDiGetDeviceRegistryPropertyW(devinfo, devinfo_data, property, None, None, Some(&mut requiredsize));
        if let Err(e) = r {
            if e.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() {
                return Err(e.into());
            }
        }

        let mut value = vec![0; requiredsize as usize];

        SetupDiGetDeviceRegistryPropertyW(
            devinfo,
            devinfo_data as *const _ as _,
            property,
            None,
            Some(&mut value),
            None,
        )?;

        let value = value.as_ptr() as *const u16;
        let value = PCWSTR(value)
            .to_string()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        Ok(value)
    }
}

pub fn build_driver_info_list(
    devinfo: HDEVINFO,
    devinfo_data: &mut SP_DEVINFO_DATA,
    driver_type: SETUP_DI_DRIVER_TYPE,
) -> io::Result<()> {
    unsafe {
        SetupDiBuildDriverInfoList(devinfo, Some(devinfo_data), driver_type)?;
    }
    Ok(())
}

pub fn destroy_driver_info_list(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    driver_type: SETUP_DI_DRIVER_TYPE,
) -> io::Result<()> {
    unsafe {
        SetupDiDestroyDriverInfoList(devinfo, Some(devinfo_data), driver_type)?;
    }
    Ok(())
}

pub fn get_driver_info_detail(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    drvinfo_data: &SP_DRVINFO_DATA_V2_W,
) -> io::Result<SP_DRVINFO_DETAIL_DATA_W2> {
    let mut drvinfo_detail: SP_DRVINFO_DETAIL_DATA_W2 = unsafe { mem::zeroed() };
    drvinfo_detail.cbSize = mem::size_of::<SP_DRVINFO_DETAIL_DATA_W>() as _;

    unsafe {
        SetupDiGetDriverInfoDetailW(
            devinfo,
            Some(devinfo_data),
            drvinfo_data,
            Some(&mut drvinfo_detail as *mut _ as _),
            mem::size_of_val(&drvinfo_detail) as _,
            None,
        )?;
    }
    Ok(drvinfo_detail)
}

pub fn set_selected_driver(
    devinfo: HDEVINFO,
    devinfo_data: &mut SP_DEVINFO_DATA,
    drvinfo_data: &mut SP_DRVINFO_DATA_V2_W,
) -> io::Result<()> {
    unsafe {
        SetupDiSetSelectedDriverW(devinfo, Some(devinfo_data), Some(drvinfo_data))?;
    }
    Ok(())
}

pub fn set_class_install_params(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    params: &SP_CLASSINSTALL_HEADER,
) -> io::Result<()> {
    unsafe {
        SetupDiSetClassInstallParamsW(devinfo, Some(devinfo_data), Some(params), mem::size_of_val(params) as _)?;
    }
    Ok(())
}

pub fn call_class_installer(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    install_function: DI_FUNCTION,
) -> io::Result<()> {
    unsafe {
        SetupDiCallClassInstaller(install_function, devinfo, Some(devinfo_data))?;
    }
    Ok(())
}

pub fn open_dev_reg_key(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    scope: u32,
    hw_profile: u32,
    key_type: u32,
    sam_desired: u32,
) -> io::Result<HKEY> {
    let key = unsafe { SetupDiOpenDevRegKey(devinfo, devinfo_data, scope, hw_profile, key_type, sam_desired)? };
    Ok(key)
}

pub fn notify_change_key_value(
    key: HKEY,
    watch_subtree: BOOL,
    notify_filter: u32,
    milliseconds: u32,
) -> io::Result<()> {
    unsafe {
        let event = CreateEventW(None, FALSE, FALSE, None)?;
        RegNotifyChangeKeyValue(key, watch_subtree, REG_NOTIFY_FILTER(notify_filter), event, TRUE).ok()?;
        match WaitForSingleObject(event, milliseconds) {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::Error::new(io::ErrorKind::TimedOut, "Registry timed out")),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

pub fn enum_driver_info(
    devinfo: HDEVINFO,
    devinfo_data: &SP_DEVINFO_DATA,
    driver_type: SETUP_DI_DRIVER_TYPE,
    member_index: u32,
) -> Option<io::Result<SP_DRVINFO_DATA_V2_W>> {
    let mut drvinfo_data: SP_DRVINFO_DATA_V2_W = unsafe { mem::zeroed() };
    drvinfo_data.cbSize = mem::size_of_val(&drvinfo_data) as _;

    let result = unsafe {
        SetupDiEnumDriverInfoW(
            devinfo,
            Some(devinfo_data),
            driver_type,
            member_index,
            &mut drvinfo_data,
        )
    };
    match result {
        Ok(_) => Some(Ok(drvinfo_data)),
        Err(e) => {
            if e.code() == ERROR_NO_MORE_ITEMS.into() {
                None
            } else {
                Some(Err(e.into()))
            }
        }
    }
}

pub fn enum_device_info(devinfo: HDEVINFO, member_index: u32) -> Option<io::Result<SP_DEVINFO_DATA>> {
    let mut devinfo_data: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
    devinfo_data.cbSize = mem::size_of_val(&devinfo_data) as _;

    match unsafe { SetupDiEnumDeviceInfo(devinfo, member_index, &mut devinfo_data) } {
        Ok(_) => Some(Ok(devinfo_data)),
        Err(e) => {
            if e.code() == ERROR_NO_MORE_ITEMS.into() {
                None
            } else {
                Some(Err(e.into()))
            }
        }
    }
}

pub fn device_io_control(
    handle: HANDLE,
    io_control_code: u32,
    in_buffer: &impl Copy,
    out_buffer: &mut impl Copy,
) -> io::Result<()> {
    let mut junk = 0;
    unsafe {
        DeviceIoControl(
            handle,
            io_control_code,
            Some(in_buffer as *const _ as _),
            mem::size_of_val(in_buffer) as _,
            Some(out_buffer as *mut _ as _),
            mem::size_of_val(out_buffer) as _,
            Some(&mut junk),
            None,
        )?;
    }
    Ok(())
}

/// Wrapper around `HDEVINFO` for freeing it on drop
// Use `Owned` instead when next version of windows crate gets published
pub struct DeviceInfo(HDEVINFO);

impl DeviceInfo {
    pub fn new(hdeviceinfo: HDEVINFO) -> Self {
        Self(hdeviceinfo)
    }

    pub fn device_iter(&self) -> DeviceInfoIter {
        DeviceInfoIter::new(self.0)
    }
}

impl Deref for DeviceInfo {
    type Target = HDEVINFO;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for DeviceInfo {
    fn drop(&mut self) {
        let _ = destroy_device_info_list(self.0);
    }
}

pub struct DeviceInfoIter {
    handle: HDEVINFO,
    current: u32,
}

impl DeviceInfoIter {
    pub fn new(handle: HDEVINFO) -> Self {
        Self { handle, current: 0 }
    }
}

impl Iterator for DeviceInfoIter {
    type Item = io::Result<SP_DEVINFO_DATA>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = enum_device_info(self.handle, self.current);
        self.current += 1;
        result
    }
}
