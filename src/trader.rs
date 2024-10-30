pub mod transaction {
    use libloading;
    use libc;
    use std::ptr;
    use std::ffi::CStr;
    use std::ffi::CString;
    use libloading::{Library, Symbol};
    use libc::{c_char, c_long, c_ulong, c_double};
    use tracing::{info, error};
    use tracing_subscriber;


    // Этот тип может иметь разную ширину в зависимости от платформы, 
    // поэтому используем cfg для определения типа.
    #[cfg(target_pointer_width = "64")]
    type IntPtr = i64;
    
    type SubsribeOrders = unsafe extern "C" fn(*const c_char, *const c_char) -> c_long;


    /// Синхронная отправка транзакции. При синхронной отправке возврат из функции происходит 
    /// только после получения результата выполнения транзакции, либо после разрыва связи 
    /// терминала QUIK с сервером.
    pub fn send_sync_transaction(lib: &Library, transaction_str: &str) -> bool {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_SEND_SYNC_TRANSACTION в библиотеке
            let send_sync_transaction: Symbol<unsafe extern "C" fn(
                *const c_char,
                *mut c_long,
                *mut c_ulong,
                *mut c_double,
                *mut c_char,
                c_ulong,
                *mut c_long,
                *mut c_char,
                c_ulong
            ) -> c_long> = lib.get(b"TRANS2QUIK_SEND_SYNC_TRANSACTION\0").expect("Could not find function");
            
            // Вызываем функцию
            let transaction_string = CString::new(transaction_str).expect("CString::new failed");
            let mut reply_code: c_long = 0;
            let mut trans_id: c_ulong = 0;
            let mut order_num: c_double = 0.0;
            let mut result_message = vec![0 as c_char; 256];
            let mut error_code: c_long = 0;
            let mut error_message = vec![0 as c_char; 256];

            let result = send_sync_transaction(
                transaction_string.as_ptr(),
                &mut reply_code as &mut c_long,
                &mut trans_id as &mut c_ulong,
                &mut order_num as &mut c_double,
                result_message.as_mut_ptr(),
                result_message.len() as c_ulong,
                &mut error_code as &mut c_long,
                error_message.as_mut_ptr(),
                error_message.len() as c_ulong,
            );

            let result_message = CStr::from_ptr(result_message.as_ptr()).to_string_lossy().into_owned();
            let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

            match result {
                0 => {
                    info!("TRANS2QUIK_SUCCESS - транзакция успешно отправлена на сервер");
                    info!("Result message: {}, transaction ID: {}", result_message, trans_id);
                },
                5 => {
                    info!("TRANS2QUIK_WRONG_SYNTAX - строка транзакции заполнена неверно");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                },
                7 => {
                    info!("TRANS2QUIK_DLL_NOT_CONNECTED - отсутствует соединение между библиотекой Trans2QUIK.dll и терминалом QUIK");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                6 => {
                    info!(" TRANS2QUIK_QUIK_NOT_CONNECTED - отсутствует соединение между терминалом QUIK и сервером");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                1 => {
                    info!("TRANS2QUIK_FAILED - транзакцию отправить не удалось.");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                _ => info!("Unknown result code"),
            }

            if result == 0
            {
                return true
            }
            else
            {
                return false
            }
        }
    }


    /// Прототип функции обратного вызова для обработки полученной информации о транзакции.
    /// Внимание! Подача асинхронных транзакций с использованием функции 
    /// обратного вызова и синхронных транзакций одновременно запрещена. 
    /// Это связано с тем, что невозможно корректно вызвать функцию обратного 
    /// вызова в момент, когда функция обработки синхронной транзакции еще 
    /// не закончила свою работу.
    pub fn transaction_reply_callback(lib: &Library) -> bool {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_TRANSACTION_REPLY_CALLBACK в библиотеке
            let transaction_reply_callback: Symbol<unsafe extern "C" fn(
                *mut c_long,
                *mut c_long,
                *mut c_ulong,
                *mut c_double,
                *mut c_char,
                *mut IntPtr
            ) -> c_long> = lib.get(b"TRANS2QUIK_TRANSACTION_REPLY_CALLBACK\0").expect("Could not find function");
            
            // Вызываем функцию
            let mut error_code: c_long = 0;
            let mut reply_code: c_long = 0;
            let mut trans_id: c_ulong = 0;
            let mut order_num: c_double = 0.0;
            let mut reply_message = vec![0 as c_char; 256];
            let mut reply_descriptor: IntPtr = 0;

            let result = transaction_reply_callback(
                &mut error_code as &mut c_long,
                &mut reply_code as &mut c_long,
                &mut trans_id as &mut c_ulong,
                &mut order_num as &mut c_double,
                reply_message.as_mut_ptr(),
                &mut reply_descriptor as &mut IntPtr
            );

            let reply_message = CStr::from_ptr(reply_message.as_ptr()).to_string_lossy().into_owned();

            match result {
                0 => {
                    info!("TRANS2QUIK_SUCCESS - транзакция передана успешно");
                    info!("Reply message: {}, reply code: {}, transaction ID: {}", reply_message, reply_code, trans_id);
                },
                7 => {
                    info!("TRANS2QUIK_DLL_NOT_CONNECTED - отсутствует соединение между библиотекой Trans2QUIK.dll и терминалом QUIK");
                }
                6 => {
                    info!(" TRANS2QUIK_QUIK_NOT_CONNECTED - отсутствует соединение между терминалом QUIK и сервером");
                }
                1 => {
                    info!("TRANS2QUIK_FAILED - транзакцию отправить не удалось.");
                    info!("Error code: {}", error_code);
                }
                _ => info!("Unknown result code"),
            }

            if result == 0
            {
                return true
            }
            else
            {
                return false
            }
        }
    }


    /// Асинхронная передача транзакции. При отправке асинхронной транзакции возврат 
    /// из функции происходит сразу же, а результат выполнения транзакции сообщается через 
    /// соответствующую функцию обратного вызова.
    pub fn send_async_transaction(lib: &Library, transaction_str: &str) -> bool {
        // Определяем тип функции
        unsafe {
            // Найдем функцию  TRANS2QUIK_SEND_ASYNC_TRANSACTION в библиотеке
            let send_async_transaction: Symbol<unsafe extern "C" fn(
                *const c_char,
                *mut c_long,
                *mut c_char,
                c_ulong
            ) -> c_long> = lib.get(b"TRANS2QUIK_SEND_ASYNC_TRANSACTION\0").expect("Could not find function");
            
            // Вызываем функцию
            let transaction_string = CString::new(transaction_str).expect("CString::new failed");
            let mut error_code: c_long = 0;
            let mut error_message = vec![0 as c_char; 256];

            let result = send_async_transaction(
                transaction_string.as_ptr(),
                &mut error_code as &mut c_long,
                error_message.as_mut_ptr(),
                error_message.len() as c_ulong,
            );

            let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

            match result {
                0 => info!("TRANS2QUIK_SUCCESS - транзакция успешно отправлена на сервер"),
                5 => {
                    info!("TRANS2QUIK_WRONG_SYNTAX - строка транзакции заполнена неверно");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                },
                7 => {
                    info!("TRANS2QUIK_DLL_NOT_CONNECTED - отсутствует соединение между библиотекой Trans2QUIK.dll и терминалом QUIK");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                6 => {
                    info!(" TRANS2QUIK_QUIK_NOT_CONNECTED - отсутствует соединение между терминалом QUIK и сервером");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                1 => {
                    info!("TRANS2QUIK_FAILED - транзакцию отправить не удалось.");
                    info!("Error code: {}, error message: {}", error_code, error_message);
                }
                _ => info!("Unknown result code"),
            }

            if result == 0
            {
                return true
            }
            else
            {
                return false
            }
        }
    }


    pub fn subscribe_orders(lib: &Library) -> SubsribeOrders{
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_SUBSCRIBE_ORDERS в библиотеке
            let subscribe_orders: Symbol<unsafe extern "C" fn(*const c_char, *const c_char) -> c_long> = lib.get(b"TRANS2QUIK_SUBSCRIBE_ORDERS\0").expect("Could not find function");
            
            // Вызываем функцию
            let class_code = CString::new("").expect("CString::new failed");
            let sec_codes = CString::new("").expect("CString::new failed");

            let result = subscribe_orders(
                class_code.as_ptr(),
                sec_codes.as_ptr(),
            );

            match result {
                0 => info!("TRANS2QUIK_SUCCESS -  подписка проведена успешно"),
                7 => info!("TRANS2QUIK_DLL_NOT_CONNECTED -  не установлена связь библиотеки Trans2QUIK.dll с терминалом QUIK"),
                6 => info!("TRANS2QUIK_QUIK_NOT_CONNECTED -  не установлена связь между Рабочим местом QUIK и сервером."),
                1 => info!("TRANS2QUIK_FAILED - попытка подписки завершилась неуспешно"),
                _ => info!("Unknown result code"),
            }

            return *subscribe_orders
        }
    }

    pub fn start_orders(lib: &Library) {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_START_ORDERS в библиотеке
            let start_orders: Symbol<unsafe extern "C" fn(SubsribeOrders)> = lib.get(b"TRANS2QUIK_START_ORDERS\0").expect("Could not find function");
            start_orders(subscribe_orders(&lib));
        }
    }
}