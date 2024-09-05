use anyhow::Result;
use connector_arrow::api::{ResultReader, Statement};
use connector_arrow::arrow;
use connector_arrow::arrow::record_batch::RecordBatch;
use futures::{AsyncRead, AsyncWrite};

fn read_to_batch<'a>(reader: impl ResultReader<'a>) -> Result<RecordBatch> {
    let batches = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
    let schema = batches.first().unwrap().schema_ref();
    Ok(arrow::compute::concat_batches(schema, &batches)?)
}

pub(crate) trait DbProtocol: Send {
    fn query(&mut self, sql: &str) -> Result<RecordBatch>;
    fn execute(&mut self, sql: &str) -> Result<()>;
}

impl DbProtocol for connector_arrow::sqlite::SQLiteConnection {
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

impl DbProtocol for connector_arrow::duckdb::DuckDBConnection {
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

#[cfg(feature = "test-dbs-external")]
pub(crate) mod external {
    use super::*;

    impl DbProtocol for connector_arrow::postgres::PostgresConnection {
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

    impl DbProtocol for connector_arrow::mysql::MySQLConnection<::mysql::Conn> {
        fn query(&mut self, sql: &str) -> Result<RecordBatch> {
            let mut statement = connector_arrow::api::Connector::query(self, sql)?;
            let reader = statement.start([])?;
            read_to_batch(reader)
        }

        fn execute(&mut self, sql: &str) -> Result<()> {
            use mysql::prelude::Queryable;
            self.inner_mut().query_iter(sql)?;
            Ok(())
        }
    }

    impl<S: AsyncRead + AsyncWrite + Unpin + Send> DbProtocol
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
