pub mod transaction {
    use libc;
    use libc::{c_char, c_double, c_long, c_ulong};
    use libloading;
    use libloading::{Library, Symbol};
    use std::ffi::CStr;
    use std::ffi::CString;
    use std::ptr;
    use tracing::{error, info};
    use tracing_subscriber;

    // Этот тип может иметь разную ширину в зависимости от платформы,
    // поэтому используем cfg для определения типа.
    #[cfg(target_pointer_width = "64")]
    type IntPtr = i64;

    type SubsribeOrders = unsafe extern "C" fn(*const c_char, *const c_char) -> c_long;

    pub fn subscribe_orders(lib: &Library) -> SubsribeOrders {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_SUBSCRIBE_ORDERS в библиотеке
            let subscribe_orders: Symbol<
                unsafe extern "C" fn(*const c_char, *const c_char) -> c_long,
            > = lib
                .get(b"TRANS2QUIK_SUBSCRIBE_ORDERS\0")
                .expect("Could not find function");

            // Вызываем функцию
            let class_code = CString::new("").expect("CString::new failed");
            let sec_codes = CString::new("").expect("CString::new failed");

            let result = subscribe_orders(class_code.as_ptr(), sec_codes.as_ptr());

            match result {
                0 => info!("TRANS2QUIK_SUCCESS -  подписка проведена успешно"),
                7 => info!("TRANS2QUIK_DLL_NOT_CONNECTED -  не установлена связь библиотеки Trans2QUIK.dll с терминалом QUIK"),
                6 => info!("TRANS2QUIK_QUIK_NOT_CONNECTED -  не установлена связь между Рабочим местом QUIK и сервером."),
                1 => info!("TRANS2QUIK_FAILED - попытка подписки завершилась неуспешно"),
                _ => info!("Unknown result code"),
            }

            return *subscribe_orders;
        }
    }

    pub fn start_orders(lib: &Library) {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_START_ORDERS в библиотеке
            let start_orders: Symbol<unsafe extern "C" fn(SubsribeOrders)> = lib
                .get(b"TRANS2QUIK_START_ORDERS\0")
                .expect("Could not find function");
            start_orders(subscribe_orders(&lib));
        }
    }
}
