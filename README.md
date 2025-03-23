## Application for algorithmic trading on the MOEX via the QUIK terminal.
## Prerequisites
* Create a table "Current Trades" in QUIK and configure ODBC output to the current_trades PostgreSQL table by matching the column names as indicated below

| Column in QUIK    | Column in Postgres |
|-------------------|--------------------|
| Инструмент[Класс] | instrument_class   |
| Класс             | class              |
| Код класса        | class_code         |
| Инструмент сокр.  | instr_short_name   |
| Код инструмента   | sec_code           |
| Сессия            | session_status     |
| Статус            | instrument_status  |
| Кратность лота    | lot_multiplier     |
| Лот               | lot_size           |
| Цена послед.      | last_price         |
| Оборот посл       | last_volume        |
| Время послед.     | last_price_time    |
| Дата торгов       | trade_date         |

* Create a `config.yaml` file
```
touch "config.yaml"
```
* Add in your `config.yaml` file
```yaml
path_to_lib: C:\QUIK\trans2quik.dll
path_to_quik: C:\QUIK\
class_code: TQBR
sec_code: 
instrument_status: торгуется
timeframe: 5
short_num_of_candles: 8
long_num_of_candles: 21
hysteresis_percentage: 0.0001
hysteresis_periods: 1
tg_token: <your_token>
psql_conn_str: host=localhost user=postgres dbname=postgres password=password
```