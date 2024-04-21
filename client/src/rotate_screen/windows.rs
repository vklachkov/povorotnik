use super::Orientation;
use anyhow::{bail, Context};
use std::{
    ffi::CStr,
    mem::{size_of, swap, zeroed},
    num::NonZeroUsize,
    ptr::null,
};
use windows_sys::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExA, EnumDisplayDevicesA, EnumDisplaySettingsA, CDS_UPDATEREGISTRY,
    DEVMODEA, DEVMODE_DISPLAY_ORIENTATION, DISPLAY_DEVICEA, DMDO_180, DMDO_270, DMDO_90,
    DMDO_DEFAULT, ENUM_CURRENT_SETTINGS,
};

pub fn rotate(display: NonZeroUsize, orientation: Orientation) -> anyhow::Result<()> {
    let display_device = get_display_device(display)
        .with_context(|| format!("getting info about display {display}"))?;

    // SAFETY: `DISPLAY_DEVICEA.DeviceName` should be valid pointer to nul-terminated string.
    let device_name = unsafe { CStr::from_ptr(display_device.DeviceName.as_ptr().cast()) };

    let mut display_mode = enum_display_settings(device_name)
        .with_context(|| format!("enumerating display {display} settings"))?;

    // Windows requires to manually swap width and height
    // if the orientation is changed from portrait to landscape or vice versa.
    if is_need_to_swap_dimensions(&display_mode, orientation) {
        swap(
            &mut display_mode.dmPelsWidth,
            &mut display_mode.dmPelsHeight,
        );
    }

    display_mode.Anonymous1.Anonymous2.dmDisplayOrientation =
        orientation_to_display_mode(orientation);

    change_display_mode(device_name, &display_mode)
        .with_context(|| "changing display {display} settings")?;

    Ok(())
}

fn get_display_device(display: NonZeroUsize) -> anyhow::Result<DISPLAY_DEVICEA> {
    let mut display_device = DISPLAY_DEVICEA {
        cb: size_of::<DISPLAY_DEVICEA>() as _,

        // SAFETY: structure can be zeroed because it will be filled correctly
        // by `EnumDisplayDevicesA`.
        ..unsafe { zeroed() }
    };

    // SAFETY: `EnumDisplayDevicesA` is safe when `lpdisplaydevice` is
    // valid structure pointer with initialized `cb` field.
    let status =
        unsafe { EnumDisplayDevicesA(null(), (display.get() - 1) as _, &mut display_device, 0) };

    if status == 0 {
        bail!("display number {display} is greater than connected displays");
    }

    Ok(display_device)
}

fn enum_display_settings(device_name: &CStr) -> anyhow::Result<DEVMODEA> {
    // SAFETY: structure can be zeroed because it will be filled correctly
    // by `EnumDisplaySettingsA`.
    let mut display_mode = unsafe { zeroed() };

    // SAFETY: `EnumDisplaySettingsA` is safe when `lpszdevicename` is valid
    // pointer to nul-terminated string and `lpdevmode` is valid structure pointer.
    let status = unsafe {
        EnumDisplaySettingsA(
            device_name.as_ptr().cast(),
            ENUM_CURRENT_SETTINGS,
            &mut display_mode,
        )
    };

    if status == 0 {
        bail!("EnumDisplaySettingsA returns {status}");
    }

    Ok(display_mode)
}

fn is_need_to_swap_dimensions(display_mode: &DEVMODEA, orientation: Orientation) -> bool {
    let display_mode_orientation =
        unsafe { display_mode.Anonymous1.Anonymous2.dmDisplayOrientation };

    let is_changed_to_portrait = [DMDO_DEFAULT, DMDO_180].contains(&display_mode_orientation)
        && [Orientation::D90, Orientation::D270].contains(&orientation);

    let is_changed_to_landscape = [DMDO_90, DMDO_270].contains(&display_mode_orientation)
        && [Orientation::D0, Orientation::D180].contains(&orientation);

    is_changed_to_portrait || is_changed_to_landscape
}

fn orientation_to_display_mode(orientation: Orientation) -> DEVMODE_DISPLAY_ORIENTATION {
    match orientation {
        Orientation::D0 => DMDO_DEFAULT,
        Orientation::D90 => DMDO_90,
        Orientation::D180 => DMDO_180,
        Orientation::D270 => DMDO_270,
    }
}

fn change_display_mode(device_name: &CStr, display_mode: &DEVMODEA) -> anyhow::Result<()> {
    // SAFETY: `ChangeDisplaySettingsExA` is safe when `lpszdevicename` is valid
    // pointer to nul-terminated string and `lpdevmode` is valid structure pointer.
    let status = unsafe {
        ChangeDisplaySettingsExA(
            device_name.as_ptr().cast(),
            display_mode,
            0,
            CDS_UPDATEREGISTRY,
            null(),
        )
    };

    if status != 0 {
        // TODO: Return error name instead of numeric value: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changedisplaysettingsexa#return-value.
        bail!("ChangeDisplaySettingsExA returns {status}");
    }

    Ok(())
}
