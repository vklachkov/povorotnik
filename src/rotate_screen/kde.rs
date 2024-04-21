use super::Orientation;
use anyhow::Context;
use std::process::Command;

pub fn rotate(display: &str, orientation: Orientation) -> anyhow::Result<()> {
    let rotation = orientation_name(orientation);

    Command::new("kscreen-doctor")
        .arg(format!("output.{display}.rotation.{rotation}"))
        .output()
        .context("executing kscreen-doctor")?;

    Ok(())
}

fn orientation_name(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::D0 => "normal",
        Orientation::D90 => "left",
        Orientation::D180 => "inverted",
        Orientation::D270 => "right",
    }
}
