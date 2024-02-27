use std::{io::IoSliceMut, mem::MaybeUninit};

use bytes::BytesMut;
use quinn_udp::{RecvMeta, UdpSocketState, UdpState};

#[derive(Debug)]
pub(crate) struct UdpSocket {
    socket: std::net::UdpSocket,
    state: UdpSocketState,
    capabilities: UdpState,
    receive_buffer: Box<[u8]>,
}

impl UdpSocket {
    pub(crate) fn new(socket: std::net::UdpSocket, max_udp_payload_size: u64) -> Result<Self> {
        let state = UdpSocketState::new();
        let capabilities = UdpState::new();
        state.configure(socket.into()).map(|_| Self {
            socket,
            state,
            capabilities,
            receive_buffer: vec![
                0;
                max_udp_payload_size.min(64 * 1024) as usize
                    * capabilities.gro_segments()
                    * quinn_udp::BATCH_SIZE
            ],
        })
    }

    pub(crate) fn max_gso_segments(&self) -> usize {
        self.capabilities.max_gso_segments()
    }

    pub(crate) fn receive(&mut self, handle: impl FnMut(RecvMeta, BytesMut)) -> Result<()> {
        let mut metas = [RecvMeta::default(); quinn_udp::BATCH_SIZE];
        let mut iovs = MaybeUninit::<[IoSliceMut<'_>; quinn_udp::BATCH_SIZE]>::uninit();

        for (i, buf) in self
            .receive_buffer
            .chunks_mut(self.receive_buffer.len() / quinn_udp::BATCH_SIZE)
            .enumerate()
        {
            // SAFETY: The `chunk_size` calculation for `chunks_mut` above ensures that `i` doesn't exceed the bounds of the array
            unsafe {
                iovs.as_mut_ptr()
                    .cast::<IoSliceMut>()
                    .add(i)
                    .write(IoSliceMut::<'_>::new(buf));
            }
        }

        // SAFETY: The above loop ensures that the entire array is initialised
        let mut iovs = unsafe { iovs.assume_init() };

        loop {
            match self.state.recv(self.socket, &mut iovs, &mut metas) {
                Ok(msgs) => {
                    for (meta, buf) in metas.iter().zip(iovs.iter()).take(msgs) {
                        let mut data: BytesMut = buf[0..meta.len].into();
                        while !data.is_empty() {
                            let buf = data.split_to(meta.stride.min(data.len()));
                            handle(meta, buf);
                        }
                    }
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => return Ok(()),
                    // Ignore ECONNRESET as it's undefined in QUIC and may be injected by an attacker
                    std::io::ErrorKind::ConnectionReset => continue,
                    _ => return Err(e),
                },
            }
        }
    }
}
