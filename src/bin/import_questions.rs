use std::{env, error::Error, fs, path::PathBuf, sync::Arc};

use sqlx::postgres::PgPoolOptions;
use xiaoluoquiz::application::{QuestionManagementError, QuestionManagementService};
use xiaoluoquiz::domain::QuestionImportBatch;
use xiaoluoquiz::server::{PgQuestionStore, config::Config};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("批量导入失败：{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let path = env::args_os().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "用法：xiaoluoquiz-import-questions <questions.json>",
        )
    })?;
    let content = fs::read_to_string(PathBuf::from(path))?;
    let batch: QuestionImportBatch = serde_json::from_str(&content)?;
    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await?;
    let service = QuestionManagementService::new(Arc::new(PgQuestionStore::new(pool)));

    match service.import_add_only(batch).await {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Err(QuestionManagementError::ImportValidation(report)) => {
            println!("{}", serde_json::to_string_pretty(&report)?);
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "批量导入包含无效题目，未写入任何题目",
            )
            .into())
        }
        Err(error) => Err(Box::new(error)),
    }
}
