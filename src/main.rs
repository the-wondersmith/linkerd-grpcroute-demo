use futures::{FutureExt, TryFuture, TryFutureExt};
use rand::Rng;
use tokio::time::Duration;

use crate::emojivoto::{
    voting_service_client::VotingServiceClient as EmojiVotingClient, VoteRequest,
};

mod emojivoto {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src",
        "/gen",
        "/emojivoto.v1.rs"
    ));
}

#[derive(Clone, Copy, Debug)]
enum TargetEmoji {
    Joy,
    Ghost,
}

#[tokio::main]
async fn main() -> Result<(), std::process::ExitCode> {
    init_tracing().map_err(|error| {
        tracing::error!(?error);
        std::process::ExitCode::from(1)
    })?;

    EmojiVotingClient::connect("http://joy-voting-svc.emojivoto.svc.cluster.local:8080")
        .and_then(|_| {
            EmojiVotingClient::connect("http://ghost-voting-svc.emojivoto.svc.cluster.local:8080")
        })
        .await
        .map_err(|error| {
            tracing::error!(?error);
            std::process::ExitCode::from(2)
        })?;

    vote().await.map_err(|error| {
        tracing::error!(?error);
        std::process::ExitCode::from(3)
    })
}

fn init_tracing() -> anyhow::Result<(), tracing::subscriber::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_thread_names(true)
            .without_time()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".parse().unwrap()),
            )
            .finish(),
    )
}

#[tracing::instrument]
async fn vote() -> anyhow::Result<()> {
    let mut client =
        EmojiVotingClient::connect("http://voting-svc.emojivoto.svc.cluster.local:8080").await?;
    let target = std::env::var("VOTE_FOR")?.parse::<TargetEmoji>()?;

    loop {
        let response = match target {
            TargetEmoji::Joy => client.vote_joy(tonic::Request::new(VoteRequest {})).await,
            TargetEmoji::Ghost => client.vote_ghost(tonic::Request::new(VoteRequest {})).await,
        }?;

        let nap_length = rand::thread_rng().gen_range(5u64..30);

        tracing::info!(
            ?target,
            ?response,
            "vote cast, voting again in {} seconds ...",
            &nap_length
        );

        tokio::time::sleep(Duration::from_secs(nap_length)).await;
    }
}

impl core::fmt::Display for TargetEmoji {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Joy => "joy",
            Self::Ghost => "ghost",
        })
    }
}

impl std::str::FromStr for TargetEmoji {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "🤣" | "joy" => Ok(Self::Joy),
            "👻" | "ghost" => Ok(Self::Ghost),
            value => anyhow::bail!("unrecognized voting target: {value}"),
        }
    }
}
