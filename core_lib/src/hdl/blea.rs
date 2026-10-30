use std::sync::Arc;
use std::time::Duration;

use bluer::adv::Advertisement;
use bluer::UuidExt;
use bytes::Bytes;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const SERVICE_DATA: Bytes = Bytes::from_static(&[
    252, 18, 142, 1, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 191, 45, 91, 160, 225, 216, 117, 36, 202, 0,
]);

const INNER_NAME: &str = "BleAdvertiser";

#[derive(Debug, Clone)]
pub struct BleAdvertiser {
    adapter: Arc<bluer::Adapter>,
}

impl BleAdvertiser {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        Ok(Self {
            adapter: Arc::new(adapter),
        })
    }

    pub async fn run(&self, ctk: CancellationToken) -> Result<(), anyhow::Error> {
        info!(
            "{INNER_NAME}: advertising on Bluetooth adapter {} with address {}",
            self.adapter.name(),
            self.adapter.address().await?
        );

        let service_uuid = Uuid::from_u16(0xFE2C);
        let handle = self
            .adapter
            .advertise(self.get_advertisment(service_uuid, SERVICE_DATA))
            .await?;
        ctk.cancelled().await;
        info!("{INNER_NAME}: tracker cancelled, returning");
        drop(handle);

        Ok(())
    }

    fn get_advertisment(&self, service_uuid: Uuid, adv_data: Bytes) -> Advertisement {
        fast_initiation_advertisement(service_uuid, adv_data)
    }
}

// Android scans for the "fast initiation" advertisement with a low duty cycle
// in the background. With the kernel default interval (0x0800 = 1.28 s) a
// Samsung phone took 1-2 minutes to notice us; 100-150 ms gives it roughly
// ten times more chances per scan window.
const ADV_MIN_INTERVAL: Duration = Duration::from_millis(100);
const ADV_MAX_INTERVAL: Duration = Duration::from_millis(150);

pub fn fast_initiation_advertisement(service_uuid: Uuid, adv_data: Bytes) -> Advertisement {
    Advertisement {
        advertisement_type: bluer::adv::Type::Broadcast,
        service_data: [(service_uuid, adv_data.into())].into(),
        min_interval: Some(ADV_MIN_INTERVAL),
        max_interval: Some(ADV_MAX_INTERVAL),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_initiation_advertisement_uses_a_short_interval() {
        let uuid = Uuid::from_u16(0xFE2C);
        let adv = fast_initiation_advertisement(uuid, SERVICE_DATA);

        assert_eq!(adv.advertisement_type, bluer::adv::Type::Broadcast);
        assert!(adv.service_data.contains_key(&uuid));
        assert_eq!(adv.min_interval, Some(Duration::from_millis(100)));
        assert_eq!(adv.max_interval, Some(Duration::from_millis(150)));
    }
}
