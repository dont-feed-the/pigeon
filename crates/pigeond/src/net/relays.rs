//! The group relay (ADR 0016, 09 §9.5): a roster's `relay` replaces n0's
//! relays on an endpoint that runs with n0's, as `[network] relays = "n0"`,
//! the default, sets it up. A person's own choice (`off`, or relay URLs)
//! wins over the groups'. With several groups the endpoint uses every
//! group's relay; with none it goes back to n0's.

use std::sync::Arc;

use iroh::RelayUrl;

use super::{NetError, Node, Reach};

impl Node {
    /// Uses the relays `wanted`, the groups' own, in place of the ones it
    /// began with, or those again when `wanted` is empty; nothing changes
    /// unless it began with n0's. Answers whether the relays changed.
    pub async fn use_group_relays(&self, wanted: &[String]) -> Result<bool, NetError> {
        if self.reach != Reach::N0 {
            return Ok(false);
        }
        let mut target = Vec::new();
        for url in wanted {
            let url: RelayUrl = url
                .parse()
                .map_err(|e| NetError::Bind(format!("relay {url}: {e}")))?;
            if !target.contains(&url) {
                target.push(url);
            }
        }
        if target.is_empty() {
            target.clone_from(&self.defaults);
        }
        let mut now = self.relays.lock().await;
        let mut changed = false;
        for url in target.iter().filter(|u| !now.contains(u)) {
            self.endpoint
                .insert_relay(url.clone(), Arc::new(url.clone().into()))
                .await;
            changed = true;
        }
        for url in now.iter().filter(|u| !target.contains(u)) {
            self.endpoint.remove_relay(url).await;
            changed = true;
        }
        *now = target;
        Ok(changed)
    }

    /// The relays the endpoint uses now.
    pub async fn relay_urls(&self) -> Vec<String> {
        self.relays
            .lock()
            .await
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NetConfig;

    fn config(reach: Reach) -> NetConfig {
        NetConfig {
            secret_key: [7; 32],
            bind: "127.0.0.1:0".parse().unwrap(),
            mdns: false,
            reach,
        }
    }

    #[tokio::test]
    async fn a_group_relay_replaces_n0s_and_goes_when_no_group_has_one() {
        let node = Node::bind(&config(Reach::N0)).await.unwrap();
        let n0 = node.relay_urls().await;
        assert!(!n0.is_empty(), "n0's relays to begin with");
        let own = vec!["https://relay.example.org".to_owned()];
        assert!(node.use_group_relays(&own).await.unwrap());
        assert_eq!(node.relay_urls().await, vec!["https://relay.example.org/"]);
        assert!(!node.use_group_relays(&own).await.unwrap(), "no change");
        assert!(node.use_group_relays(&[]).await.unwrap());
        assert_eq!(node.relay_urls().await, n0);
        assert!(node.use_group_relays(&["no url".to_owned()]).await.is_err());
        node.close().await;
    }

    #[tokio::test]
    async fn the_persons_choice_wins_over_the_groups() {
        let mine = vec!["https://mine.example.org".to_owned()];
        for reach in [Reach::Local, Reach::Relays(mine.clone())] {
            let node = Node::bind(&config(reach)).await.unwrap();
            let before = node.relay_urls().await;
            let own = vec!["https://relay.example.org".to_owned()];
            assert!(!node.use_group_relays(&own).await.unwrap());
            assert_eq!(node.relay_urls().await, before);
            node.close().await;
        }
    }
}
