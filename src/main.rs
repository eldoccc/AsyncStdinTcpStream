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
    let channel_to_server = mpsc::channel::<String>(32);
    let channel_to_client = mpsc::channel::<String>(32);
    let (tx_mpsc_network, rx_mpsc_network) = channel_to_server;
    let (tx_process, rx_process) = channel_to_client;
    let tx_mpsc_stdin = tx_mpsc_network.clone();

    //net_listen
    let handle = spawn(async move {
        handle_connexion(listener, tx_mpsc_network, rx_process).await;
    });
    //input
    let stdin_listen = spawn(async move {
        stdin_listen(tx_mpsc_stdin).await;
    });
    //process
    let process_manager = spawn(async move {
        process_message(rx_mpsc_network, tx_process).await;
    });
    handle.await.unwrap();
    stdin_listen.await.unwrap();
    process_manager.await.unwrap();
}

//net_listen
async fn handle_connexion(
    listener: TcpListener,
    mut tx_mpsc_network: Sender<String>,
    mut rx_process: Receiver<String>,
) {
    loop {
        let (stream, _) = listener.accept().await.expect("accept panic");
        println!("Connection established");
        let (reading_part, writing_part) = stream.into_split();
        let (channel_com_w, channel_com_r) = mpsc::channel::<bool>(32);

        let res = tokio::join!(
            network_listen(reading_part, tx_mpsc_network, channel_com_w),
            network_write(writing_part, rx_process, channel_com_r)
        );
        tx_mpsc_network = res.0;
        rx_process = res.1;
    }
}
//net_rx
async fn network_listen(
    mut stream: OwnedReadHalf,
    tx_mpsc: Sender<String>,
    writer: Sender<bool>,
) -> Sender<String> {
    //read from network and send to process

    loop {
        let mut buf = [0; 1024];
        let _ = match stream.read(&mut buf).await {
            Ok(0) => {
                writer.send(true).await.unwrap();
                println!("Connection closed");
                break;
            }
            Ok(n) => {
                let res = std::str::from_utf8(&buf[..n]).unwrap().to_string();
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
    tx_mpsc
}
//input
async fn stdin_listen(tx_mpsc: Sender<String>) {
    let mut stdin = BufReader::new(stdin());
    loop {
        let mut line = String::new();
        stdin.read_line(&mut line).await.unwrap();
        tx_mpsc
            .try_send(line)
            .map_err(|e| eprintln!("stdin listen error: {}", e))
            .unwrap();
    }
}
//process
async fn process_message(mut rx_mpsc: Receiver<String>, tx_process: Sender<String>) {
    loop {
        match rx_mpsc.recv().await {
            Some(s) => tx_process.try_send(s).unwrap(),
            None => println!("Error: channel closed"),
        };
    }
}
//net_tx
async fn network_write(
    mut stream: OwnedWriteHalf,
    mut rx_process: Receiver<String>,
    mut reader: Receiver<bool>,
) -> Receiver<String> {
    //read from process and send to network

    loop {
        //check if stream has been closed an breaks out of the loop if this is the case
        tokio::select! {
            val_exit = reader.recv() => {
                if val_exit.unwrap() == true {
                    break;
                }
            }
            val_response = rx_process.recv() => {
                println!("message received: {}", val_response.unwrap());
                match stream.write_all("message received\n".as_bytes()).await {
                    Ok(_) => {}
                    Err(e) => println!("Error: {}", e),
                }
            }
        }
    }
    rx_process
}
