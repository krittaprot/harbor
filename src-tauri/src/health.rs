use crate::model::{Health, Service, TunnelHealth};
use std::{
    collections::BTreeMap,
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

pub fn classify(code: u16) -> &'static str {
    if code < 500 { "Ok" } else { "Failed" }
}

pub fn tunnel_port(service: &Service, pid: u32) -> bool {
    service.processes.iter().any(|p| {
        p.pid == pid
            && (p.name.contains("cloudflared")
                || p.argv.first().is_some_and(|a| a.ends_with("/cloudflared")))
    })
}

fn probe(service: &Service) -> Health {
    let mut result = Health {
        status: "NotChecked".into(),
        checked_at: Some(chrono::Utc::now()),
        ..Default::default()
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            result.reason = Some(e.to_string());
            return result;
        }
    };
    for port in service.ports.iter().filter(|p| tunnel_port(service, p.pid)) {
        let response = client
            .get(format!("http://127.0.0.1:{}/ready", port.port))
            .send();
        let (ready, reason) = match response {
            Ok(r) => {
                let code = r.status().as_u16();
                let body = r.json::<serde_json::Value>().ok();
                let ready = code == 200
                    && body.as_ref().is_some_and(|b| {
                        b["status"] == 200 && b["readyConnections"].as_u64().unwrap_or(0) > 0
                    });
                (
                    ready,
                    if ready {
                        "Tunnel connected".into()
                    } else {
                        format!("/ready {code}: no verified ready connections")
                    },
                )
            }
            Err(e) => (false, e.to_string()),
        };
        result.tunnels.push(TunnelHealth {
            port: port.port,
            ready,
            reason,
        });
    }
    let port = service
        .ports
        .iter()
        .filter(|p| !tunnel_port(service, p.pid))
        .min_by_key(|p| p.port);
    let started = Instant::now();
    if service.url.is_none() && port.is_some_and(|p| [5432, 21118].contains(&p.port)) {
        let p = port.unwrap();
        let address: SocketAddr = format!("127.0.0.1:{}", p.port).parse().unwrap();
        match TcpStream::connect_timeout(&address, Duration::from_secs(2)) {
            Ok(_) => result.status = "PortOpen".into(),
            Err(e) => {
                result.status = "Failed".into();
                result.reason = Some(e.to_string());
            }
        }
    } else if let Some(url) = service
        .url
        .clone()
        .or_else(|| port.map(|p| format!("http://127.0.0.1:{}/", p.port)))
    {
        match client.get(&url).send() {
            Ok(r) => {
                result.code = Some(r.status().as_u16());
                result.status = classify(r.status().as_u16()).into();
                result.reason = Some(url);
            }
            Err(e) => {
                result.status = "Failed".into();
                result.reason = Some(e.to_string());
            }
        }
    }
    result.ms = Some(started.elapsed().as_millis() as u64);
    if result.tunnels.iter().any(|t| !t.ready) {
        result.status = "Failed".into();
        result.reason = Some("A Cloudflare tunnel is not ready".into());
    } else if result.status == "NotChecked" && !result.tunnels.is_empty() {
        result.status = "Ok".into();
    }
    result
}

#[derive(Default)]
pub struct HealthScanner {
    cache: BTreeMap<String, (Instant, String, Health)>,
}

impl HealthScanner {
    pub fn update(&mut self, services: &mut [Service]) {
        let mut pending = vec![];
        for (i, service) in services.iter_mut().enumerate().filter(|(_, s)| !s.apple) {
            let identity = format!(
                "{:?}{:?}{:?}",
                service.ports,
                service.url,
                service.jobs.iter().map(|j| j.pid).collect::<Vec<_>>()
            );
            if let Some((time, previous, health)) = self.cache.get(&service.id)
                && time.elapsed() < Duration::from_secs(30)
                && previous == &identity
            {
                service.health = health.clone();
                continue;
            }
            pending.push((i, identity));
        }
        // Bounded concurrent work; no network probes run on the UI thread.
        for batch in pending.chunks(12) {
            let results = std::thread::scope(|scope| {
                let handles: Vec<_> = batch
                    .iter()
                    .map(|(i, identity)| {
                        let service = &services[*i];
                        (*i, identity.clone(), scope.spawn(move || probe(service)))
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|(i, identity, h)| (i, identity, h.join().unwrap_or_default()))
                    .collect::<Vec<_>>()
            });
            for (i, identity, health) in results {
                self.cache.insert(
                    services[i].id.clone(),
                    (Instant::now(), identity, health.clone()),
                );
                services[i].health = health;
            }
        }
        self.cache
            .retain(|id, _| services.iter().any(|s| &s.id == id));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn redirects_and_auth_responses_are_healthy() {
        for code in [200, 302, 401, 404, 499] {
            assert_eq!(super::classify(code), "Ok");
        }
        for code in [500, 502, 503] {
            assert_eq!(super::classify(code), "Failed");
        }
    }
}
