use std::sync::OnceLock;

use anyhow::{bail, Result};
use tiberius::numeric::BigDecimal;
use tiberius::time::time::PrimitiveDateTime;
use tiberius::{AuthMethod, Client, ColumnType, Config};
use tokio::net::TcpStream;
use tokio::runtime;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::DbProtocolHandler;
use crate::dbs::Row;

fn runtime() -> &'static runtime::Runtime {
    static RUNTIME: OnceLock<runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    })
}

pub fn init() -> Box<dyn DbProtocolHandler> {
    let mut config = Config::new();
    config.host("localhost");
    config.port(1433);
    config.trust_cert();
    config.authentication(AuthMethod::sql_server("sa", "Wordpass123##"));

    let res = runtime().block_on(async {
        let tcp = TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true).unwrap();
        Client::connect(config, tcp.compat_write()).await
    });
    Box::new(res.unwrap())
}

impl DbProtocolHandler for tiberius::Client<Compat<TcpStream>> {
    fn query(&mut self, sql: &str) -> Result<Vec<Row>> {
        runtime().block_on(async {
            let mut stream = self.query(sql, &[]).await?;
            let mut vec = vec![];
            let cols_option = stream.columns().await?;
            if cols_option.is_none() {
                return Ok(vec);
            }
            let cols = cols_option.unwrap().to_vec();
            for row in stream.into_first_result().await.unwrap() {
                let mut columns = vec![];
                for (i, col) in cols.iter().enumerate() {
                    let value = match col.column_type() {
                        ColumnType::Null => String::new(),
                        ColumnType::Bit => String::from(row.get::<&str, usize>(i).unwrap()),
                        ColumnType::Intn | ColumnType::Int4 => row
                            .get::<i32, usize>(i)
                            .map_or_else(String::new, |i| i.to_string()),
                        ColumnType::Floatn => vec![
                            row.try_get::<f32, usize>(i).map(|o| o.map(f64::from)),
                            row.try_get::<f64, usize>(i),
                        ]
                        .into_iter()
                        .find(|r| r.is_ok())
                        .unwrap()
                        .unwrap()
                        .map_or_else(String::new, |i| i.to_string()),
                        ColumnType::Numericn | ColumnType::Decimaln => row
                            .get::<BigDecimal, usize>(i)
                            .map(|d| d.normalized())
                            .unwrap()
                            .to_string(),
                        ColumnType::BigVarChar | ColumnType::NVarchar => {
                            String::from(row.get::<&str, usize>(i).unwrap_or(""))
                        }
                        ColumnType::Datetimen => {
                            row.get::<PrimitiveDateTime, usize>(i).unwrap().to_string()
                        }
                        typ => bail!("mssql type {:?}", typ),
                    };
                    columns.push(value);
                }
                vec.push(columns);
            }

            Ok(vec)
        })
    }
}
