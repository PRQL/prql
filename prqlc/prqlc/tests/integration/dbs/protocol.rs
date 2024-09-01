use anyhow::Result;
use connector_arrow::api::{ResultReader, Statement};
use connector_arrow::arrow;
use connector_arrow::arrow::record_batch::RecordBatch;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "params")]
pub enum DbProtocol {
    DuckDb,
    MsSql,
    MySql { url: String },
    Postgres { url: String },
    SQLite,
}

pub trait DbProtocolHandler: Send {
    fn query(&mut self, sql: &str) -> Result<RecordBatch>;

    fn execute(&mut self, sql: &str) -> Result<()>;
}

#[cfg(feature = "test-dbs")]
pub mod sqlite {
    use super::*;

    #[allow(dead_code)]
    pub fn init() -> Box<dyn DbProtocolHandler> {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let conn_ar = connector_arrow::sqlite::SQLiteConnection::new(conn);

        Box::new(conn_ar)
    }

    impl DbProtocolHandler for connector_arrow::sqlite::SQLiteConnection {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            self.inner_mut().execute(sql, ())?;
            Ok(())
        }
    }
}

#[cfg(feature = "test-dbs")]
pub mod duckdb {
    use super::*;

    #[allow(dead_code)]
    pub fn init() -> Box<dyn DbProtocolHandler> {
        let conn = ::duckdb::Connection::open_in_memory().unwrap();
        let conn_ar = connector_arrow::duckdb::DuckDBConnection::new(conn);

        Box::new(conn_ar)
    }

    impl DbProtocolHandler for connector_arrow::duckdb::DuckDBConnection {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            self.inner_mut().execute(sql, [])?;
            Ok(())
        }
    }
}

#[cfg(feature = "test-dbs-external")]
pub mod postgres {
    use super::*;

    #[allow(dead_code)]
    pub fn init(url: &str) -> Box<dyn DbProtocolHandler> {
        use connector_arrow::postgres::PostgresConnection;

        let client = ::postgres::Client::connect(url, ::postgres::NoTls).unwrap();
        Box::new(PostgresConnection::new(client))
    }

    impl DbProtocolHandler for connector_arrow::postgres::PostgresConnection {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            self.inner_mut().execute(sql, &[])?;
            Ok(())
        }
    }
}

#[cfg(feature = "test-dbs-external")]
pub mod mysql {
    use super::*;
    use ::mysql::prelude::Queryable;

    #[allow(dead_code)]
    pub fn init(url: &str) -> Box<dyn DbProtocolHandler> {
        let conn = ::mysql::Conn::new(url)
            .unwrap_or_else(|e| panic!("Failed to connect to {}:\n{}", url, e));

        Box::new(connector_arrow::mysql::MySQLConnection::<::mysql::Conn>::new(conn))
    }

    impl DbProtocolHandler for connector_arrow::mysql::MySQLConnection<::mysql::Conn> {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            self.inner_mut().query_iter(sql)?;
            Ok(())
        }
    }
}

#[cfg(feature = "test-dbs-external")]
pub mod mssql {
    use super::*;
    use futures::{AsyncRead, AsyncWrite};
    use std::sync::Arc;

    #[allow(dead_code)]
    pub fn init() -> Box<dyn DbProtocolHandler> {
        use tokio_util::compat::TokioAsyncWriteCompatExt;

        let mut config = tiberius::Config::new();
        config.host("localhost");
        config.port(1433);
        config.trust_cert();
        config.authentication(tiberius::AuthMethod::sql_server("sa", "Wordpass123##"));

        let rt = runtime();

        let client = rt
            .block_on(async {
                let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
                tcp.set_nodelay(true).unwrap();
                tiberius::Client::connect(config, tcp.compat_write()).await
            })
            .unwrap();
        Box::new(connector_arrow::tiberius::TiberiusConnection::new(
            rt, client,
        ))
    }

    fn runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        )
    }

    impl<S: AsyncRead + AsyncWrite + Unpin + Send> DbProtocolHandler
        for connector_arrow::tiberius::TiberiusConnection<S>
    {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            let (rt, client) = self.inner_mut();
            rt.block_on(client.execute(sql, &[]))?;
            Ok(())
        }
    }
}

fn read_to_batch<'a>(reader: impl ResultReader<'a>) -> Result<RecordBatch> {
    let batches = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
    let schema = batches.first().unwrap().schema_ref();
    Ok(arrow::compute::concat_batches(schema, &batches)?)
}
