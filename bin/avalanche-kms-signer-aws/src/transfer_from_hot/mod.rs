use std::io;

use avalanche_types::{
    jsonrpc::client::{evm as json_client_evm, info as json_client_info},
    key, wallet,
};
use clap::{value_parser, Arg, Command};
use primitive_types::{H160, U256};

pub const NAME: &str = "transfer-from-hot";

pub fn command() -> Command {
    Command::new(NAME)
        .about("Transfers the native tokens 'from' hot key to the 'to' address")
        .arg(
            Arg::new("LOG_LEVEL")
                .long("log-level")
                .short('l')
                .help("Sets the log level")
                .required(false)
                .num_args(1)
                .value_parser(["debug", "info"])
                .default_value("info"),
        )
        .arg(
            Arg::new("CHAIN_RPC_URL")
                .long("chain-rpc-url")
                .help("Sets to fetch other information from the RPC endpoints (e.g., balances)")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("TRANSFERER_KEY")
                .long("transferer-key")
                .help("Sets the from private key (in hex format)")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("TRANSFER_AMOUNT")
                .long("transfer-amount")
                .help("Sets the transfer amount")
                .required(true)
                .value_parser(value_parser!(u64))
                .num_args(1),
        )
        .arg(
            Arg::new("TRANSFEREE_ADDRESS")
                .long("transferee-address")
                .help("Sets the transferee address")
                .required(true)
                .num_args(1),
        )
}

pub async fn execute(
    log_level: &str,
    chain_rpc_url: &str,
    transferer_key: &str,
    transfer_amount: U256,
    transferee_addr: H160,
) -> io::Result<()> {
    // ref. https://github.com/env-logger-rs/env_logger/issues/47
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level),
    );

    let resp = json_client_info::get_network_id(chain_rpc_url)
        .await
        .unwrap();
    let network_id = resp.result.unwrap().network_id;

    let chain_id = json_client_evm::chain_id(chain_rpc_url).await.unwrap();
    log::info!("running against {chain_rpc_url}, network Id {network_id}, chain Id {chain_id}");

    let transferer_key = key::secp256k1::private_key::Key::from_hex(transferer_key).unwrap();
    let transferer_key_info = transferer_key.to_info(network_id).unwrap();
    log::info!("created hot key:\n\n{}\n", transferer_key_info);

    log::info!(
        "transfering {transfer_amount} from {} to {transferee_addr} via {chain_rpc_url}",
        transferer_key_info.eth_address
    );
    let transferer_key_signer: ethers_signers::LocalWallet =
        transferer_key.to_ethers_core_signing_key().into();

    let w = wallet::Builder::new(&transferer_key)
        .base_http_url(chain_rpc_url.to_string())
        .build()
        .await?;
    let transferer_evm_wallet =
        w.evm(&transferer_key_signer, chain_rpc_url, U256::from(chain_id))?;

    let transferer_balance = transferer_evm_wallet.balance().await?;
    println!(
        "transferrer {} balance: {}",
        transferer_key_info.eth_address, transferer_balance
    );
    let transferee_balance = json_client_evm::get_balance(chain_rpc_url, transferee_addr).await?;
    println!(
        "transferrer 0x{:x} balance: {}",
        transferee_addr, transferee_balance
    );

    let tx_id = transferer_evm_wallet
        .eip1559()
        .recipient(transferee_addr)
        .value(transfer_amount)
        .urgent()
        .check_acceptance(true)
        .submit()
        .await?;
    log::info!("evm ethers wallet SUCCESS with transaction id {}", tx_id);

    Ok(())
}
