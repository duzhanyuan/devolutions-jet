use crate::interceptor::MessageReader;
use crate::interceptor::UnknownMessageReader;
use crate::interceptor::{PacketInterceptor, PeerInfo};
use log::{debug, error};
use packet::builder::Builder;
use packet::ether::Builder as BuildEthernet;
use packet::ether::Protocol;
use packet::ip::v6::Builder as BuildV6;
use packet::tcp::flag::Flags;
use pcap_file::PcapWriter;
use std::fs::File;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

const TCP_IP_PACKET_MAX_SIZE: usize = 16384;

#[derive(Clone)]
pub struct PcapInterceptor {
    pcap_writer: Arc<Mutex<PcapWriter<File>>>,
    server_info: Arc<Mutex<PeerInfo>>,
    client_info: Arc<Mutex<PeerInfo>>,
    message_reader: Arc<Box<MessageReader>>,
}

impl PcapInterceptor {
    pub fn new(server_addr: SocketAddr, client_addr: SocketAddr, pcap_filename: &str) -> Self {
        let file = File::create(pcap_filename).expect("Error creating file");
        let pcap_writer: PcapWriter<File> = PcapWriter::new(file).expect("Error creating pcap writer");

        PcapInterceptor {
            server_info: Arc::new(Mutex::new(PeerInfo::new(server_addr))),
            client_info: Arc::new(Mutex::new(PeerInfo::new(client_addr))),
            pcap_writer: Arc::new(Mutex::new(pcap_writer)),
            message_reader: Arc::new(Box::new(UnknownMessageReader::get_messages)),
        }
    }

    pub fn set_message_reader<F: 'static + Fn(&mut Vec<u8>) -> Vec<Vec<u8>> + Send + Sync>(
        &mut self,
        message_reader: F,
    ) {
        self.message_reader = Arc::new(Box::new(message_reader));
    }
}

impl PacketInterceptor for PcapInterceptor {
    fn on_new_packet(&mut self, source_addr: Option<SocketAddr>, data: &[u8]) {
        debug!("New packet intercepted. Packet size = {}", data.len());

        let mut server_info = self.server_info.lock().unwrap();
        let mut client_info = self.client_info.lock().unwrap();
        let is_from_server = source_addr.unwrap() == server_info.addr;

        let (messages, source_addr, dest_addr, seq_number, ack_number) = if is_from_server {
            server_info.data.append(&mut data.to_vec());
            (
                (self.message_reader)(&mut server_info.data),
                server_info.addr,
                client_info.addr,
                &mut client_info.sequence_number,
                server_info.sequence_number,
            )
        } else {
            client_info.data.append(&mut data.to_vec());
            (
                (self.message_reader)(&mut client_info.data),
                client_info.addr,
                server_info.addr,
                &mut server_info.sequence_number,
                client_info.sequence_number,
            )
        };

        for data in messages {
            for data_chunk in data.chunks(TCP_IP_PACKET_MAX_SIZE) {
                // Build tcpip packet
                let tcpip_packet = match (source_addr, dest_addr) {
                    (SocketAddr::V4(source), SocketAddr::V4(dest)) => {
                        BuildEthernet::default()
                            .destination([0x00, 0x15, 0x5D, 0x01, 0x64, 0x04].into())
                            .unwrap() // 00:15:5D:01:64:04
                            .source([0x00, 0x15, 0x5D, 0x01, 0x64, 0x01].into())
                            .unwrap() // 00:15:5D:01:64:01
                            .protocol(Protocol::Ipv4)
                            .unwrap()
                            .ip()
                            .unwrap()
                            .v4()
                            .unwrap()
                            .source(*source.ip())
                            .unwrap()
                            .destination(*dest.ip())
                            .unwrap()
                            .ttl(128)
                            .unwrap()
                            .tcp()
                            .unwrap()
                            .window(0x7fff)
                            .unwrap()
                            .source(source_addr.port())
                            .unwrap()
                            .destination(dest_addr.port())
                            .unwrap()
                            .acknowledgment(ack_number)
                            .unwrap()
                            .sequence(*seq_number)
                            .unwrap()
                            .flags(Flags::from_bits_truncate(0x0018))
                            .unwrap()
                            .payload(data_chunk)
                            .unwrap()
                            .build()
                            .unwrap()
                    }
                    (SocketAddr::V6(_source), SocketAddr::V6(_dest)) => BuildV6::default().build().unwrap(),
                    (_, _) => unreachable!(),
                };

                // Write packet in pcap file
                let since_epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards");
                let mut pcap_writer = self.pcap_writer.lock().unwrap();
                if let Err(e) = pcap_writer.write(
                    since_epoch.as_secs() as u32,
                    since_epoch.subsec_micros(),
                    tcpip_packet.as_ref(),
                ) {
                    error!("Error writting pcap file: {}", e);
                }

                // Update the seq_number
                *seq_number += data_chunk.len() as u32;
            }
        }
    }
}
