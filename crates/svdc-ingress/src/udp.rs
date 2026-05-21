//! WBS-2.1 — UDP multicast subscriber.
//!
//! Receives L2-stripped SV payloads from the simulator process
//! (`ssiec-sv-publisher udp ...`) or any UDP-tunnelled SV source.
//! Implements [`Subscriber`] so the existing decode → ring → aligner
//! pipeline does not change.
//!
//! Wire shape: each datagram carries a single SV frame **starting
//! at the APPID** (the L2 Ethernet header and any 802.1Q VLAN tag
//! have been stripped). The decoder entry point is
//! `ssiec_sv_publisher::decode_l2_stripped_frame`.
//!
//! Real production ingest is AF_PACKET (Phase 5). The UDP path is
//! the **bench / simulator** transport per ADR-0015.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use crate::subscriber::{Subscriber, SubscriberError};
use crate::IngressTimestamp;

/// UDP subscriber bound to a multicast group or a plain UDP socket.
#[derive(Debug)]
pub struct UdpSubscriber {
    sock: UdpSocket,
    /// Reusable receive buffer; reset on every `next_frame`.
    buf: Vec<u8>,
}

impl UdpSubscriber {
    /// Bind to `addr` and, if the address is in the IPv4 multicast
    /// range (224.0.0.0/4), join the group on the default interface.
    /// Returns the socket wrapped as a [`Subscriber`].
    ///
    /// `recv_timeout` controls how long `next_frame` blocks waiting
    /// for a datagram. `None` blocks indefinitely; a short timeout
    /// (e.g. `Duration::from_millis(250)`) lets the daemon shut down
    /// cleanly. The receive loop reports
    /// [`SubscriberError::Closed`] when the timeout fires so the
    /// caller can re-check shutdown flags.
    pub fn bind(addr: SocketAddr, recv_timeout: Option<Duration>) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(addr)?;
        if let SocketAddr::V4(v4) = addr {
            let ip = *v4.ip();
            if ip.is_multicast() {
                sock.join_multicast_v4(&ip, &Ipv4Addr::UNSPECIFIED)?;
            }
        }
        if let Some(t) = recv_timeout {
            sock.set_read_timeout(Some(t))?;
        }
        Ok(Self {
            sock,
            buf: vec![0u8; 4096],
        })
    }

    /// Local socket address the subscriber is bound to. Useful in
    /// tests that bind an ephemeral port and need the assigned port
    /// number.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.sock.local_addr()
    }
}

impl Subscriber for UdpSubscriber {
    fn next_frame(&mut self) -> Result<(Vec<u8>, IngressTimestamp), SubscriberError> {
        match self.sock.recv_from(self.buf.as_mut_slice()) {
            Ok((n, _from)) => {
                let ts = IngressTimestamp::now();
                Ok((self.buf[..n].to_vec(), ts))
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // Timeout — let the caller re-check its shutdown
                // flag and call us again.
                Err(SubscriberError::Closed)
            }
            Err(_) => Err(SubscriberError::Closed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddrV4;

    #[test]
    fn bind_unicast_loopback_yields_a_datagram() {
        // Bind subscriber on loopback, ephemeral port.
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let mut sub = UdpSubscriber::bind(addr, Some(Duration::from_millis(500))).unwrap();
        let bound = sub.local_addr().unwrap();

        // Sender: same loopback, ephemeral source port.
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
        tx.send_to(b"hello-sv-payload", bound).unwrap();

        let (payload, ts) = sub.next_frame().unwrap();
        assert_eq!(payload, b"hello-sv-payload");
        assert!(ts.unix_ns() > 0);
    }

    #[test]
    fn empty_socket_times_out_as_closed() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let mut sub = UdpSubscriber::bind(addr, Some(Duration::from_millis(50))).unwrap();
        let r = sub.next_frame();
        assert!(matches!(r, Err(SubscriberError::Closed)));
    }
}
