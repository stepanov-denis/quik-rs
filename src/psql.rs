//! # It works with exchange data from the PostgreSQL DBMS.
use bb8::RunError;
use bb8_postgres::{bb8::Pool, tokio_postgres::NoTls, PostgresConnectionManager};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::error;

#[derive(Debug)]
pub struct DataForEma {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

pub struct Db {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl Db {
    // Инициализация пула соединений
    pub async fn new(
        connection_str: &str,
    ) -> Result<Self, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Create the manager
        let manager = PostgresConnectionManager::new_from_stringlike(connection_str, NoTls)
            .map_err(|e| {
                error!("Error creating PostgresConnectionManager: {:?}", e);
                e
            })?;

        // Build the connection pool
        let pool = Pool::builder()
            .max_size(15)
            .min_idle(Some(1))
            .build(manager)
            .await
            .map_err(|e| {
                error!("Error building connection pool: {:?}", e);
                e
            })?;

        Ok(Db { pool })
    }

    // Создание таблицы текущих торгов
    pub async fn create_current_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Получаем соединение из пула
        let conn = self.pool.get().await.map_err(|e| {
            error!("Ошибка получения соединения из пула: {:?}", e);
            e
        })?;

        // Создаем таблицу
        let query = "
            CREATE TABLE IF NOT EXISTS current_trades (
                instrument_class VARCHAR(200),
                class VARCHAR(128),
                class_code VARCHAR(12),
                instrument VARCHAR(20),
                instrument_code VARCHAR(12),
                session_status VARCHAR(32),
                instrument_status VARCHAR(32),
                lot_multiplier INTEGER,
                lot INTEGER,
                last_price  DECIMAL(15,6),
                last_volume DECIMAL(15,6),
                last_price_time TIME,
                trade_date DATE
            );
        ";

        // Выполняем команду создания таблицы
        conn.execute(query, &[]).await.map_err(|e| {
            error!(
                "Ошибка выполнения запроса создания таблицы current_trades: {:?}",
                e
            );
            e
        })?;

        Ok(())
    }

    // Создание таблицы исторических данных торгов
    pub async fn create_historical_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Получаем соединение из пула
        let conn = self.pool.get().await.map_err(|e| {
            error!("Ошибка получения соединения из пула: {:?}", e);
            e
        })?;

        // Создаем таблицу
        let query = "
            CREATE TABLE IF NOT EXISTS historical_trades (
                id SERIAL PRIMARY KEY,
                instrument_class VARCHAR(200),
                class VARCHAR(128),
                class_code VARCHAR(12),
                instrument VARCHAR(20),
                instrument_code VARCHAR(12),
                session_status VARCHAR(32),
                instrument_status VARCHAR(32),
                lot_multiplier INTEGER,
                lot INTEGER,
                last_price DECIMAL(15,6),
                last_volume DECIMAL(15,6),
                last_price_time TIME,
                trade_date DATE,
                update_timestamptz TIMESTAMPTZ DEFAULT NOW()
            );
        ";

        // Выполняем команду создания таблицы
        conn.execute(query, &[]).await.map_err(|e| {
            error!(
                "Ошибка выполнения запроса создания таблицы historical_trades: {:?}",
                e
            );
            e
        })?;

        Ok(())
    }

    // Функция триггера, которая будет вставлять данные в таблицу historical_trades
    pub async fn insert_into_historical(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Получаем соединение из пула
        let conn = self.pool.get().await.map_err(|e| {
            error!("Ошибка получения соединения из пула: {:?}", e);
            e
        })?;

        // Создаем функцию триггера
        let query = "
            CREATE OR REPLACE FUNCTION insert_into_historical()
            RETURNS TRIGGER AS $$
            BEGIN
                INSERT INTO historical_trades (
                    instrument_class,
                    class,
                    class_code,
                    instrument,
                    instrument_code,
                    session_status,
                    instrument_status,
                    lot_multiplier,
                    lot,
                    last_price,
                    last_volume,
                    last_price_time,
                    trade_date
                ) VALUES (
                    NEW.instrument_class,
                    NEW.class,
                    NEW.class_code,
                    NEW.instrument,
                    NEW.instrument_code,
                    NEW.session_status,
                    NEW.instrument_status,
                    NEW.lot_multiplier,
                    NEW.lot,
                    NEW.last_price,
                    NEW.last_volume,
                    NEW.last_price_time,
                    NEW.trade_date
                );
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        ";

        // Выполняем команду создания функции триггера
        conn.execute(query, &[]).await.map_err(|e| {
            error!("Ошибка выполнения запроса создания функции триггера insert_into_historical(): {:?}", e);
            e
        })?;

        Ok(())
    }

    // Триггер, который будет срабатывать перед обновлением данных в таблице current_trades
    pub async fn before_update_current_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Получаем соединение из пула
        let conn = self.pool.get().await.map_err(|e| {
            error!("Ошибка получения соединения из пула: {:?}", e);
            e
        })?;

        // Создаем триггер
        let query = "
            DO $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1 FROM pg_trigger
                    WHERE tgname = 'before_update_current_trades'
                ) THEN
                    CREATE TRIGGER before_update_current_trades
                    BEFORE UPDATE ON current_trades
                    FOR EACH ROW
                    EXECUTE FUNCTION insert_into_historical();
                END IF;
            END;
            $$ LANGUAGE plpgsql;
        ";

        // Выполняем команду создания триггера
        conn.execute(query, &[]).await.map_err(|e| {
            error!(
                "Ошибка выполнения запроса создания триггера before_update_current_trades: {:?}",
                e
            );
            e
        })?;

        Ok(())
    }

    // Инициализация базы данных
    pub async fn init(&self) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        self.create_current_trades().await?;
        self.create_historical_trades().await?;
        self.insert_into_historical().await?;
        self.before_update_current_trades().await?;

        Ok(())
    }

    // Получение данных торгов для расчета EMA
    pub async fn get_data_for_ema(
        &self,
        instrument_code: &str,
        lookback_interval_seconds: f64,
        period_length_seconds: f64,
    ) -> Result<Vec<DataForEma>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Получаем соединение из пула
        let conn = self.pool.get().await.map_err(|e| {
            error!("Ошибка получения соединения из пула: {:?}", e);
            e
        })?;

        // SQL-запрос с использованием приведения типов
        let query = "
            WITH period_data AS (
                SELECT
                    ht.*,
                    NOW() - FLOOR(
                        EXTRACT(EPOCH FROM NOW() - ht.update_timestamptz) / $1::double precision
                    ) * $1::double precision * INTERVAL '1 second' AS period_start
                FROM
                    historical_trades ht
                WHERE
                    ht.instrument_code = $3
                    AND ht.update_timestamptz >= NOW() - $2::double precision * INTERVAL '1 second'
            )
            SELECT
                period_start,
                (ARRAY_AGG(last_price ORDER BY update_timestamptz ASC))[1] AS open_price,
                (ARRAY_AGG(last_price ORDER BY update_timestamptz DESC))[1] AS close_price,
                MIN(last_price) AS min_price,
                MAX(last_price) AS max_price,
                SUM(last_volume) AS period_volume
            FROM
                period_data
            GROUP BY
                period_start
            ORDER BY
                period_start;
        ";

        // Выполняем запрос с параметрами
        let rows = conn
            .query(
                query,
                &[
                    &period_length_seconds,
                    &lookback_interval_seconds,
                    &instrument_code,
                ],
            )
            .await
            .map_err(|e| {
                error!(
                    "Ошибка выполнения запроса получения данных для расчета EMA: {:?}",
                    e
                );
                e
            })?;

        // Печатаем названия столбцов
        println!(
            "{:<30} | {:>15} | {:>15} | {:>15} | {:>15} | {:>15}",
            "period_start", "open_price", "close_price", "min_price", "max_price", "period_volume"
        );

        // Печатаем разделительную линию
        println!(
            "{:-<30}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-",
            "", "", "", "", "", ""
        );

        // Создаем вектор для DataItem
        let mut data_item: Vec<DataForEma> = Vec::new();

        // Обрабатываем результаты
        for row in rows {
            let period_start: DateTime<Utc> = row.get("period_start");

            let open_price: f64 = row
                .try_get::<_, Decimal>("open_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let close_price: f64 = row
                .try_get::<_, Decimal>("close_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let min_price: f64 = row
                .try_get::<_, Decimal>("min_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let max_price: f64 = row
                .try_get::<_, Decimal>("max_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let period_volume: f64 = row
                .try_get::<_, Decimal>("period_volume")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            println!(
                "{:<30} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6}",
                period_start, open_price, close_price, min_price, max_price, period_volume
            );

            let item = DataForEma {
                open: open_price,
                high: max_price,
                low: min_price,
                close: close_price,
                volume: period_volume,
            };

            data_item.push(item);
        }

        Ok(data_item)
    }
}
