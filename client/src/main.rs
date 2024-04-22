mod rotate_screen;

use std::sync::Arc;

use anyhow::Context;
use btleplug::{
    api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter},
    platform::{Adapter, Manager, PeripheralId},
};
use futures::StreamExt;
use log::warn;
use rotate_screen::Orientation;
use serde::Deserialize;

const DATA_SERVICE_UUID: uuid::Uuid = uuid::uuid!("0000ff10-0000-1000-8000-00805f9b34fb");
const DATA_CHR_UUID: uuid::Uuid = uuid::uuid!("0000ff11-0000-1000-8000-00805f9b34fb");

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct Acc {
    x: f32,
    y: f32,
    z: f32,
}

#[tokio::main]
async fn main() {
    simple_logger::init().unwrap();

    log::set_max_level(log::LevelFilter::Debug);

    if let Err(err) = bluetooth_demo().await {
        log::error!("Fatal error: {err:#}");
    }
}

async fn bluetooth_demo() -> anyhow::Result<()> {
    let manager = Manager::new().await?;

    let adapter_list = manager.adapters().await?;
    let adapter = adapter_list
        .into_iter()
        .next()
        .map(Arc::new)
        .context("no Bluetooth adapter found")?;

    let mut events = adapter
        .events()
        .await
        .context("connecting to events stream")?;

    adapter
        .start_scan(ScanFilter {
            services: vec![DATA_SERVICE_UUID],
        })
        .await
        .context("starting scan")?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                log::debug!("DeviceDiscovered: '{id}'");
                device_discovered(adapter.clone(), id);
            }
            CentralEvent::DeviceConnected(id) => {
                log::debug!("DeviceConnected: '{id}'");
            }
            CentralEvent::DeviceDisconnected(id) => {
                log::debug!("DeviceDisconnected: '{id}'");
            }
            _ => {}
        }
    }

    Ok(())
}

fn device_discovered(adapter: Arc<Adapter>, id: PeripheralId) {
    tokio::spawn(async move {
        if let Err(err) = _device_discovered(adapter, id.clone()).await {
            log::warn!("Peripheral '{id}' error: {err:#}");
        }
    });
}

async fn _device_discovered(adapter: Arc<Adapter>, id: PeripheralId) -> anyhow::Result<()> {
    let peripheral = adapter
        .peripheral(&id)
        .await
        .with_context(|| format!("accessing to peripheral '{id}' info"))?;

    let properties = peripheral
        .properties()
        .await
        .with_context(|| format!("reading peripheral '{id}' properties"))?
        .with_context(|| format!("peripheral '{id}' should have properties"))?;

    let printable_name = properties.local_name.unwrap_or_else(|| id.to_string());

    peripheral
        .connect()
        .await
        .with_context(|| format!("connecting to peripheral '{printable_name}'"))?;

    peripheral
        .discover_services()
        .await
        .with_context(|| format!("discovering peripheral '{printable_name}' services"))?;

    let characteristic = peripheral
        .characteristics()
        .into_iter()
        .find(|chr| {
            chr.descriptors
                .iter()
                .any(|desc| desc.characteristic_uuid == DATA_CHR_UUID)
        })
        .with_context(|| {
            format!("characteristic '{DATA_CHR_UUID}' not found in peripheral '{printable_name}'")
        })?;

    peripheral
        .subscribe(&characteristic)
        .await
        .with_context(|| format!("subscribing to peripheral '{printable_name}' services"))?;

    let mut notifications = peripheral.notifications().await.with_context(|| {
        format!("subscribing to notifications from peripheral '{printable_name}'")
    })?;

    let mut orientation = rotate_screen::Orientation::D0;
    while let Some(notification) = notifications.next().await {
        if let Err(err) = handle_povorotnik_data(&mut orientation, notification.value) {
            warn!("Failed to process data from povorotnik: {err:#}");
        }
    }

    Ok(())
}

fn handle_povorotnik_data(orientation: &mut Orientation, data: Vec<u8>) -> anyhow::Result<()> {
    let raw = String::from_utf8(data).context("converting notification value to string")?;

    let Acc { x, y, .. } = serde_json::from_str(&raw)
        .with_context(|| format!("deserializing notification value '{raw}'"))?;

    let previous_orientation = *orientation;
    let new_orientation = get_orientation_from_accelerometer(x, y).unwrap_or(previous_orientation);

    // TODO: Remove screens hardcode
    if new_orientation != previous_orientation {
        #[cfg(target_os = "windows")]
        dbg!(rotate_screen::rotate(
            rotate_screen::Platform::Windows,
            "1",
            orientation
        ));

        #[cfg(not(target_os = "windows"))]
        dbg!(rotate_screen::rotate(
            rotate_screen::Platform::Kde,
            "eDP-1",
            new_orientation
        ));
    }

    *orientation = new_orientation;

    Ok(())
}

fn get_orientation_from_accelerometer(x: f32, y: f32) -> Option<Orientation> {
    if (-0.6..0.6).contains(&x) && (0.6..1.1).contains(&y) {
        Some(Orientation::D0)
    } else if (-1.1..-0.6).contains(&x) && (-0.6..0.6).contains(&y) {
        Some(Orientation::D90)
    } else if (-0.6..0.6).contains(&x) && (-1.1..-0.6).contains(&y) {
        Some(Orientation::D180)
    } else if (0.6..1.1).contains(&x) && (-0.6..0.6).contains(&y) {
        Some(Orientation::D270)
    } else {
        None
    }
}
