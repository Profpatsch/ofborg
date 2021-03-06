#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate ircbot;
extern crate irc;
extern crate amqp;
extern crate env_logger;

#[macro_use]
extern crate log;

use ircbot::factoids::Factoids;
use amqp::protocol::basic::Deliver;
use amqp::protocol::basic::BasicProperties;
use amqp::Basic;
use amqp::Channel;
use amqp::Session;
use amqp::Table;
use std::path::Path;

use ircbot::config;

use std::env;

#[derive(Serialize, Deserialize, Debug)]
struct MessageToIRC {
    target: String,
    body: String
}

#[derive(Serialize, Deserialize, Debug)]
struct MessageFromIRC {
    from: String,
    sender: String,
    body: String
}

fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "info");
        env_logger::init().unwrap();
        info!("Defaulting RUST_LOG environment variable to info");
    } else {
        env_logger::init().unwrap();
    }

    let cfg = config::load(env::args().nth(1).unwrap().as_ref());
    let factoids_file = env::args().nth(2).unwrap();
    let factoids = Factoids::load(&Path::new(&factoids_file)).factoids;
    println!("{:?}", factoids.get("str1"));

    let mut session = Session::open_url(&cfg.rabbitmq.as_uri()).unwrap();
    println!("Connected to rabbitmq");
    println!("About to open channel #1");
    let mut channel = session.open_channel(1).unwrap();


    let read_queue = channel.queue_declare("", false, false, true,
                                           false, false, Table::new()).unwrap();

    channel.queue_bind(read_queue.queue.as_ref(), "exchange-messages",
                       "".as_ref(), false, Table::new()).unwrap();

    let consumer_name = channel.basic_consume(
        move |chan: &mut Channel, _deliver: Deliver, _headers: BasicProperties, body: Vec<u8>| {
            debug!("Got a message");
            let msg: Result<MessageFromIRC, serde_json::Error> = serde_json::from_slice(&body);
            if let Ok(msg) = msg {
                let trigger = msg.body.trim();
                if let Some(response) = factoids.get(trigger) {
                    let resp = Some(MessageToIRC{
                        target: msg.from.clone(),
                        body: response.clone(),
                    });

                    chan.basic_publish(
                        "".to_owned(),
                        "queue-publish".to_owned(),
                        false,
                        false,
                        BasicProperties {
                            content_type: Some("application/json".to_owned()),
                            ..Default::default()
                        },
                        serde_json::to_string(&resp).unwrap().into_bytes()
                    ).expect("Failed to publish message");
                } else {
                    debug!("Message didn't match: {:?}", msg);
                }
            }
        },
        read_queue.queue.as_ref(), "", false, true, false, false, Table::new());
    println!("Starting consumer {:?}", consumer_name);

    channel.start_consuming();
    channel.close(200, "Bye").unwrap();
}
