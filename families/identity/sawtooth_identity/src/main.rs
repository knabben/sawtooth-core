/*
 * Copyright 2018 Bitwise IO Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * ------------------------------------------------------------------------------
 */

#[macro_use]
extern crate cfg_if;

cfg_if! {
     if #[cfg(target_arch = "wasm32")] {
         #[macro_use]
         extern crate sabre_sdk;
     } else {
        #[macro_use]
        extern crate clap;
        #[macro_use]
        extern crate log;
        extern crate log4rs;
        extern crate rustc_serialize;
        extern crate sawtooth_sdk;
        extern crate opentelemetry;
        use log::LevelFilter;
        use log4rs::append::console::ConsoleAppender;
        use log4rs::config::{Appender, Config, Root};
        use log4rs::encode::pattern::PatternEncoder;
        use std::process;
        use sawtooth_sdk::processor::TransactionProcessor;
        use handler::IdentityTransactionHandler;
        use opentelemetry::api::{
            Key, Provider, Sampler, Span,
            TracerGenerics,
        };
        use opentelemetry::{exporter::trace::jaeger, global, sdk};
    }
}

pub mod handler;
mod protos;
mod state;
extern crate crypto;
extern crate protobuf;

#[cfg(target_arch = "wasm32")]
fn main() {}

fn init_tracer() {
    let exporter = jaeger::Exporter::builder()
        .with_collector_endpoint("127.0.0.1:6831".parse().unwrap())
        .with_process(jaeger::Process {
            service_name: "identity-tp",
            tags: vec![Key::new("version").string("1.2.3")],
        })
        .init();
    let provider = sdk::Provider::builder()
        .with_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Sampler::Always,
            ..Default::default()
        })
        .build();
    global::set_provider(provider);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Initializing the tracer exporter
    init_tracer();

    let matches = clap_app!(identity =>
        (version: crate_version!())
        (about: "Identity Transaction Processor (Rust)")
        (@arg connect: -C --connect +takes_value "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple "increase output verbosity")
    )
    .get_matches();

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:4004");

    let console_log_level;
    match matches.occurrences_of("verbose") {
        0 => console_log_level = LevelFilter::Warn,
        1 => console_log_level = LevelFilter::Info,
        2 => console_log_level = LevelFilter::Debug,
        3 | _ => console_log_level = LevelFilter::Trace,
    }

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{h({l:5.5})} | {({M}:{L}):20.20} | {m}{n}",
        )))
        .build();

    let config = match Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(console_log_level))
    {
        Ok(x) => x,
        Err(_) => process::exit(1),
    };

    match log4rs::init_config(config) {
        Ok(_) => (),
        Err(_) => process::exit(1),
    }

    global::trace_provider()
        .get_tracer("identity")
        .with_span("boot", move |span| {
            span.add_event("Starting TP".to_string());
        });

    let handler = IdentityTransactionHandler::new();
    let mut processor = TransactionProcessor::new(endpoint);

    info!("Console logging level: {}", console_log_level);

    processor.add_handler(&handler);
    processor.start();
}
