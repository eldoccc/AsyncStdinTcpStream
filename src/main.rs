extern crate core;

use tokio::io::{stdin, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::spawn;

use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8000")
        .await
        .expect("listener panic");

    let (tx_mpsc_network, rx_mpsc_network) = mpsc::channel::<String>(32);
    let (tx_process, rx_process) = mpsc::channel::<String>(32);
    let tx_mpsc_stdin = tx_mpsc_network.clone();
    let (stream, _) = listener.accept().await.expect("accept panic");
    let (reading_part, writing_part) = stream.into_split();
    let network_listen = spawn(async move {
        network_listen(reading_part, tx_mpsc_network).await;
    });
    let stdin_listen = spawn(async move {
        stdin_listen(tx_mpsc_stdin).await;
    });
    let process_manager = spawn(async move {
        process_message(rx_mpsc_network, tx_process).await;
    });
    let network_write = spawn(async move {
        network_write(writing_part, rx_process).await;
    });

    network_listen.await.unwrap();
    stdin_listen.await.unwrap();
    process_manager.await.unwrap();
    network_write.await.unwrap();
}

async fn network_listen(mut stream: OwnedReadHalf, tx_mpsc: Sender<String>) {
    //read from network and send to process

    loop {
        let mut buf = [0; 1024];
        let _ = match stream.read(&mut buf).await {
            Ok(_) => {
                let res = std::str::from_utf8(&buf).unwrap().to_string();
                println!("Received message from network: {}", res);
                tx_mpsc
                    .try_send(res)
                    .map_err(|e| eprintln!("network listen error: {}", e))
                    .unwrap();
            }
            Err(e) => {
                println!("listen Error: {}", e);
                continue;
            }
        };
    }
}

async fn network_write(mut stream: OwnedWriteHalf, mut rx_process: Receiver<String>) {
    //read from process and send to network
    loop {
        let response = rx_process.recv().await.unwrap();
        println!("final message: {}\n", response);
        let _ = stream.write_all("message received".as_bytes()).await;
    }
}
async fn process_message(mut rx_mpsc: Receiver<String>, tx_process: Sender<String>) {
    loop {
        match rx_mpsc.recv().await {
            Some(s) => tx_process.try_send(s).unwrap(),
            None => println!("Error: channel closed"),
        };
    }
}

async fn stdin_listen(tx_mpsc: Sender<String>) {
    let mut stdin = BufReader::new(stdin());
    loop {
        let mut line = String::new();
        stdin.read_line(&mut line).await.unwrap();
        println!("Received message from stdin: {}", line);
        tx_mpsc
            .try_send(line)
            .map_err(|e| eprintln!("stdin listen error: {}", e))
            .unwrap();
    }
}
