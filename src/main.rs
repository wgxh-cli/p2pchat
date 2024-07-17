use libp2p::{
  futures::StreamExt, mdns::{
    tokio::Behaviour as MdnsBehaviour,
    Event as MdnsEvent,
  }, request_response::{
    json::Behaviour as ResqBehaviour,
    Event as ResqEvent,
    Message as ResqMessage,
    ProtocolSupport
  }, swarm::{NetworkBehaviour, SwarmEvent}, tls::Config as TlsConfig, yamux::Config as YamuxConfig, PeerId, StreamProtocol, SwarmBuilder
};
use serde::{Deserialize, Serialize};
use std::{error::Error, time::Duration};
use tokio::{io::{stdin, AsyncBufReadExt, BufReader}, select};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatReq {
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRes {}

#[derive(NetworkBehaviour)]
pub struct ChatBehaviour {
  pub mdns: MdnsBehaviour,
  pub resq: ResqBehaviour<ChatReq, ChatRes>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let mut swarm = SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(Default::default(), TlsConfig::new, YamuxConfig::default)?
    .with_behaviour(|key| {
      let mdns = MdnsBehaviour::new(Default::default(), key.public().to_peer_id())?;
      let resq = ResqBehaviour::new(
        [(StreamProtocol::new("/test-net"), ProtocolSupport::Full)],
        Default::default(),
      );

      Ok(ChatBehaviour { mdns, resq })
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(2000)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  let mut found: Vec<PeerId> = vec![];
  let connected: bool = false;
  let connected_id: Option<PeerId> = None;

  let mut stdin = BufReader::new(stdin()).lines();

  loop {
    select! {
      event = swarm.select_next_some() => {
        match event {
          SwarmEvent::Behaviour(event) => match event {
            ChatBehaviourEvent::Mdns(MdnsEvent::Discovered(nodes)) => {
              println!("Found nodes!");
              for (i, node) in nodes.iter().enumerate() {
                println!("{i}. {} - {}", node.0, node.1);
                found.push(node.0);
              }
            },
            ChatBehaviourEvent::Resq(ResqEvent::Message { peer, message }) => {
              match message {
                ResqMessage::Request { request, .. } => {
                  println!("From {peer}> {}", request.message);
                }
                _ => panic!("AA"),
              }
            }
            _ => {}
          }
          SwarmEvent::NewListenAddr { .. } => {}
          _ => {}
        }
      },
      Ok(Some(line)) = stdin.next_line() => {
        if !connected {
          if let Ok(index) = line.parse::<u8>() {
            if let Some(id) = found.get(index as usize) {
              swarm.dial(*id).unwrap();
            } else {
              println!("Please enter a valid node id!");
            }
          } else {
            println!("Please enter a valid node id!");
          }
        } else {
          swarm.behaviour_mut().resq.send_request(connected_id.as_ref().unwrap(), ChatReq {
            message: line
          });
        }
      }
    }
  }
}
