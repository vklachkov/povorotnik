mod kde;
#[cfg(target_os = "windows")]
mod win32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    #[cfg(target_os = "windows")]
    Windows,
    Kde,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    D0,
    D90,
    D180,
    D270,
}

pub fn rotate(platform: Platform, display: &str, orientation: Orientation) -> anyhow::Result<()> {
    match platform {
        #[cfg(target_os = "windows")]
        Platform::Windows => {
            let display = display
                .parse()
                .map_err(|err| anyhow::anyhow!("invalid display number: {err}"))?;

            windows::rotate(display, orientation)?;
        }

        Platform::Kde => {
            kde::rotate(display, orientation)?;
        }
    }

    Ok(())
}
