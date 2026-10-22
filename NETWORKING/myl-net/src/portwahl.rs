//! **Wählen vom Horchport, mit Ausweg** (Fund 591, 2026-10-22, gefunden im
//! Netzlauf und in der Probe `entdeckung` des Knotens).
//!
//! Jede Wahl des Swarms bindet ihren TCP-Anschluss vorgabegemäß an den
//! eigenen Horchport (`PortUse::Reuse`). Das braucht das Lochstanzen
//! (DCUtR), und es lässt die Gegenstelle den Horchport als Absender sehen.
//!
//! 📌 **Der Fund:** Zwei Knoten, die einander zugleich finden, wählen
//! einander zugleich an, beide vom Horchport zum Horchport. TCP macht
//! daraus eine einzige Verbindung, in der beide wählen, und der Handshake
//! scheitert („input error“). Danach liegt genau dieses Adressenpaar in
//! TIME_WAIT, und jede neue Wahl mit demselben Paar scheitert mit
//! „Address already in use“. Der TCP-Transport von libp2p weicht dann nur
//! bei `AddrNotAvailable` auf einen neuen Port aus; so meldet es Linux.
//! macOS und Windows melden `AddrInUse`, und dort blieben die beiden
//! Knoten einander bis zum Ablauf von TIME_WAIT unerreichbar. In der Probe
//! mit drei Knoten scheiterte so unter Last jeder achte Lauf, und im
//! Netzlauf mit fünfzehn Knoten stand die Meldung bei mehreren Knoten.
//!
//! ⚑ **Der Ausweg:** Scheitert eine Wahl vom Horchport, weil die Adresse
//! belegt oder nicht verfügbar ist, wählt diese Hülle einmal von einem
//! neuen Port. Eine Wahl, die ausdrücklich einen neuen Port verlangt, und
//! jeder andere Fehler gehen unverändert durch.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::FutureExt;
use libp2p::core::transport::{DialOpts, ListenerId, PortUse, Transport, TransportError, TransportEvent};
use libp2p::Multiaddr;

/// Ein Transport, dessen Wahl vom Horchport bei belegter Adresse von einem
/// neuen Port wiederholt wird.
#[derive(Debug)]
pub struct MitAusweichport<T>(pub T);

/// Ob ein Fehler der Wahl heisst, dass das Adressenpaar belegt ist.
pub fn adresse_belegt(fehler: &std::io::Error) -> bool {
    matches!(fehler.kind(), std::io::ErrorKind::AddrInUse | std::io::ErrorKind::AddrNotAvailable)
}

impl<T> Transport for MitAusweichport<T>
where
    T: Transport<Error = std::io::Error> + Unpin,
    T::Dial: Send + 'static,
    T::Output: Send + 'static,
{
    type Output = T::Output;
    type Error = std::io::Error;
    type ListenerUpgrade = T::ListenerUpgrade;
    type Dial = BoxFuture<'static, Result<T::Output, std::io::Error>>;

    fn listen_on(&mut self, id: ListenerId, addr: Multiaddr) -> Result<(), TransportError<Self::Error>> {
        self.0.listen_on(id, addr)
    }

    fn remove_listener(&mut self, id: ListenerId) -> bool {
        self.0.remove_listener(id)
    }

    fn dial(&mut self, addr: Multiaddr, opts: DialOpts) -> Result<Self::Dial, TransportError<Self::Error>> {
        let erste = self.0.dial(addr.clone(), opts)?;
        if opts.port_use != PortUse::Reuse {
            return Ok(erste.boxed());
        }
        // Der Ersatz wird hier schon angelegt, denn danach ist `self` nicht
        // mehr erreichbar; er wählt erst, wenn er abgefragt wird, und wird
        // sonst verworfen.
        let ersatz = self.0.dial(addr, DialOpts { port_use: PortUse::New, ..opts });
        Ok(async move {
            match erste.await {
                Err(fehler) if adresse_belegt(&fehler) => match ersatz {
                    Ok(wahl) => wahl.await,
                    Err(_) => Err(fehler),
                },
                ergebnis => ergebnis,
            }
        }
        .boxed())
    }

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        Pin::new(&mut self.get_mut().0).poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belegt_ist_belegt_und_sonst_nichts() {
        use std::io::{Error, ErrorKind};
        assert!(adresse_belegt(&Error::from(ErrorKind::AddrInUse)));
        assert!(adresse_belegt(&Error::from(ErrorKind::AddrNotAvailable)));
        assert!(!adresse_belegt(&Error::from(ErrorKind::ConnectionRefused)));
        assert!(!adresse_belegt(&Error::from(ErrorKind::TimedOut)));
    }

    /// ⚑ **Der Fall selbst:** Ein Anschluss hält das Paar Horchport zu Ziel
    /// besetzt; die Wahl vom Horchport zu demselben Ziel muss trotzdem
    /// ankommen. Nur auf Unix: Windows kennt kein `SO_REUSEPORT`, mit dem
    /// sich das Paar hier besetzen ließe.
    #[cfg(unix)]
    #[tokio::test]
    async fn die_wahl_weicht_auf_einen_neuen_port_aus() {
        let ziel = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Ziel");
        let ziel_port = ziel.local_addr().expect("Adresse").port();
        let annehmen = tokio::spawn(async move {
            let mut n = 0;
            while n < 2 {
                if ziel.accept().await.is_ok() {
                    n += 1;
                }
            }
        });
        let mut t = MitAusweichport(libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default()));
        t.listen_on(ListenerId::next(), "/ip4/127.0.0.1/tcp/0".parse().expect("Adresse")).expect("horchen");
        // Bis der Horchport feststeht.
        let horch = loop {
            match futures::future::poll_fn(|cx| Pin::new(&mut t).poll(cx)).await {
                TransportEvent::NewAddress { listen_addr, .. } => break listen_addr,
                _ => continue,
            }
        };
        let horch_port = horch
            .iter()
            .find_map(|p| match p {
                libp2p::multiaddr::Protocol::Tcp(port) => Some(port),
                _ => None,
            })
            .expect("Port");
        // Das Paar besetzen: ein Anschluss vom Horchport zum Ziel.
        let besetzt = tokio::net::TcpSocket::new_v4().expect("Anschluss");
        besetzt.set_reuseaddr(true).expect("reuseaddr");
        besetzt.set_reuseport(true).expect("reuseport");
        besetzt.bind(format!("127.0.0.1:{horch_port}").parse().expect("Adresse")).expect("binden");
        let _halten = besetzt.connect(format!("127.0.0.1:{ziel_port}").parse().expect("Adresse")).await.expect("besetzen");
        let wahl = t
            .dial(
                format!("/ip4/127.0.0.1/tcp/{ziel_port}").parse().expect("Adresse"),
                DialOpts { role: libp2p::core::Endpoint::Dialer, port_use: PortUse::Reuse },
            )
            .expect("wählen");
        let ergebnis = tokio::time::timeout(std::time::Duration::from_secs(5), wahl).await.expect("rechtzeitig");
        assert!(ergebnis.is_ok(), "die Wahl kam nicht an: {:?}", ergebnis.err());
        tokio::time::timeout(std::time::Duration::from_secs(5), annehmen).await.expect("angenommen").expect("Aufgabe");
    }
}
