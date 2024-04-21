mod rotate_screen;

use anyhow::Context;
use btleplug::{
    api::{bleuuid::BleUuid, Central, CentralEvent, Manager as _, Peripheral, ScanFilter},
    platform::Manager,
};
use futures::StreamExt;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
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
    let adapter = adapter_list.first().context("no Bluetooth adapter found")?;

    let mut events = adapter
        .events()
        .await
        .context("connecting to events stream")?;

    adapter
        .start_scan(ScanFilter::default())
        .await
        .context("starting scan")?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                let peripheral = adapter
                    .peripheral(&id)
                    .await
                    .context("accessing to peripheral info")?;

                let properties = peripheral
                    .properties()
                    .await
                    .context("reading peripheral properties")?;

                let Some(properties) = properties else {
                    continue;
                };

                let Some(name) = properties.local_name else {
                    continue;
                };

                log::debug!("DeviceDiscovered: {name}");

                if name != "LE Counter" && name != "GATT Counter" {
                    continue;
                }

                peripheral
                    .connect()
                    .await
                    .context("connecting to peripheral")?;

                peripheral
                    .discover_services()
                    .await
                    .context("discovering peripheral services")?;

                log::debug!("Characteristics: {:?}", peripheral.characteristics());

                let characteristic = peripheral
                    .characteristics()
                    .into_iter()
                    .find(|chr| {
                        chr.descriptors
                            .iter()
                            .find(|desc| {
                                desc.characteristic_uuid.to_string()
                                    == "0000ff11-0000-1000-8000-00805f9b34fb"
                            })
                            .is_some()
                    })
                    .context("LE Counter should have counter characteristic")?;

                peripheral
                    .subscribe(&characteristic)
                    .await
                    .context("subscribing to counter")?;

                let mut notifications = peripheral
                    .notifications()
                    .await
                    .context("subscribing to device notifications")?;

                let mut orientation = rotate_screen::Orientation::D0;

                while let Some(notification) = notifications.next().await {
                    log::debug!(
                        "Notification value: {:x?}",
                        std::str::from_utf8(&notification.value)
                    );

                    let Ok(acc) = serde_json::from_slice::<Acc>(&notification.value) else {
                        log::debug!("Invalid json");
                        continue;
                    };

                    log::info!("Acceleration: {acc:?}");

                    let Acc { x, y, .. } = acc;

                    let previous_orientation = orientation;
                    orientation = if (-0.6..0.6).contains(&x) && (0.6..1.1).contains(&y) {
                        rotate_screen::Orientation::D0
                    } else if (-1.1..-0.6).contains(&x) && (-0.6..0.6).contains(&y) {
                        rotate_screen::Orientation::D90
                    } else if (-0.6..0.6).contains(&x) && (-1.1..-0.6).contains(&y) {
                        rotate_screen::Orientation::D180
                    } else if (0.6..1.1).contains(&x) && (-0.6..0.6).contains(&y) {
                        rotate_screen::Orientation::D270
                    } else {
                        previous_orientation
                    };

                    if orientation != previous_orientation {
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
                            orientation
                        ));
                    }
                }
            }
            CentralEvent::DeviceConnected(id) => {
                log::debug!("DeviceConnected: {:?}", id);
            }
            CentralEvent::DeviceDisconnected(id) => {
                log::debug!("DeviceDisconnected: {:?}", id);
            }
            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                // log::debug!(
                //     "ManufacturerDataAdvertisement: {:?}, {:?}",
                //     id,
                //     manufacturer_data
                // );
            }
            CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                // log::debug!("ServiceDataAdvertisement: {:?}, {:?}", id, service_data);
            }
            CentralEvent::ServicesAdvertisement { id, services } => {
                let services: Vec<String> =
                    services.into_iter().map(|s| s.to_short_string()).collect();

                // log::debug!("ServicesAdvertisement: {:?}, {:?}", id, services);
            }
            _ => {}
        }
    }

    Ok(())
}
