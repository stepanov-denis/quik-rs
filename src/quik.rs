use std::error::Error;
use libloading;
use libc;
use std::ptr;
use std::ffi::CStr;
use std::ffi::CString;
use libloading::{Library, Symbol};
use libc::{c_char, c_long, c_ulong};
use tracing::{info, error};
use tracing_subscriber;
use std::fmt;


type TRANS2QUIK_CONNECTION_STATUS_CALLBACK = ();


pub enum Trans2quikResult {
    Success,
    TerminalNotFound,
    DllVersionNotSupported,
    DllAlreadyConnectedToQuik,
    Failed,
    Unknown,
}


pub struct Terminal {
    library: Library,
}


impl Terminal {
    /// Функция используется для загрузки библиотеки DLL
    pub fn new(path: &str) -> Result<Self, libloading::Error> {
        unsafe {
            let library = Library::new(path).map_err(|e| { error!("DLL loading error: {:?}", e); e})?;
            
            Ok(Terminal { library })
        }
    }


    /// Функция используется для установления связи с терминалом QUIK.
    pub fn trans2quik_connect(&self, connection_string: *const c_char, error_code: *mut c_long, error_message: *mut c_char, error_message_len: c_ulong) -> Result<c_long, libloading::Error> {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_CONNECT в библиотеке
            let connect: Symbol<unsafe extern "C" fn(*const c_char, *mut c_long, *mut c_char, c_ulong) -> c_long> = self.library.get(b"TRANS2QUIK_CONNECT\0").map_err(|e| { error!("TRANS2QUIK_CONNECT error: {}", e); e})?;

            Ok(connect(connection_string, error_code, error_message, error_message_len))
        }
    }


    pub fn connect(&self) -> Result<Trans2quikResult, libloading::Error> {
            // Вызываем функцию
            let connection_string = CString::new(r"c:\QUIK Junior").expect("CString::new failed");
            let mut result_code: c_long = 0;
            let mut result_message = vec![0 as c_char; 256];
            let result_message_len = result_message.len();

            let result = self.trans2quik_connect(
                connection_string.as_ptr(),
                &mut result_code as *mut c_long,
                result_message.as_mut_ptr(),
                result_message_len as c_ulong,
            )?;

            let result_message = unsafe {
                CStr::from_ptr(result_message.as_ptr()).to_string_lossy().into_owned()
            };

            match result {
                0 => {
                    info!("TRANS2QUIK_SUCCESS - соединение установлено успешно");
                    Ok(Trans2quikResult::Success)
                },
                2 => {
                    info!("Result_code: {}, message: {}", result_code, result_message);
                    Ok(Trans2quikResult::TerminalNotFound)
                },
                3 => {
                    info!("Result_code: {}, messsage: {}", result_code, result_message);
                    Ok(Trans2quikResult::DllVersionNotSupported)
                },
                4 => {
                    info!("Result_code: {}, message: {}", result_code, result_message);
                    Ok(Trans2quikResult::DllAlreadyConnectedToQuik)
                },
                1 => {
                    info!("Result_code: {}, message: {}", result_code, result_message);
                    Ok(Trans2quikResult::Failed)
                },
                _ => {
                    info!("Unknown result code: {}, message: {}", result_code, result_message);
                    Ok(Trans2quikResult::Unknown)
                },
            }
    }
}









/// Функция используется для разрыва связи с терминалом QUIK.
pub fn disconnect(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_DISCONNECT в библиотеке
        let disconnect: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_DISCONNECT\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = disconnect(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            0 => info!("TRANS2QUIK_SUCCESS - соединение библиотеки Trans2QUIK.dll с Рабочим местом QUIK разорвано успешно"),
            1 => {
                info!("TRANS2QUIK_FAILED - произошла ошибка при разрыве соединения");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            7 => {
                info!("TRANS2QUIK_DLL_NOT_CONNECTED - попытка разорвать соединение при не установленной связи");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
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


/// Функция используется для проверки наличия соединения между терминалом QUIK и сервером.
pub fn is_quik_connected(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_IS_QUIK_CONNECTED в библиотеке
        let is_quik_connected: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_IS_QUIK_CONNECTED\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = is_quik_connected(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            8 => info!("TRANS2QUIK_QUIK_CONNECTED - соединение между терминалом QUIK и сервером установлено"),
            6 => {
                info!(" TRANS2QUIK_QUIK_NOT_CONNECTED - соединение между терминалом QUIK и сервером не установлено");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            7 => {
                info!("TRANS2QUIK_DLL_NOT_CONNECTED - не установлена связь библиотеки Trans2QUIK.dll с терминалом QUIK");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            _ => info!("Unknown result code"),
        }

        if result == 8
        {
            return true
        }
        else
        {
            return false
        }
    }
}


/// Функция используется для проверки наличия соединения между библиотекой Trans2QUIK.dll и терминалом QUIK.
pub fn is_dll_connected(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_IS_DLL_CONNECTED в библиотеке
        let is_dll_connected: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_IS_DLL_CONNECTED\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = is_dll_connected(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            10 => info!(" TRANS2QUIK_DLL_CONNECTED - соединение библиотеки Trans2QUIK.dll с терминалом QUIK установлено"),
            7 => {
                info!("TRANS2QUIK_DLL_NOT_CONNECTED - не установлена связь библиотеки Trans2QUIK.dll с терминалом QUIK");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            _ => info!("Unknown result code"),
        }

        if result == 10
        {
            return true
        }
        else
        {
            return false
        }
    }
}


/// Описание прототипа Функции обратного вызова для контроля за состоянием соединения 
/// между библиотекой Trans2QUIK.dll и используемым терминалом QUIK и между 
/// используемым терминалом QUIK и сервером.
pub fn connection_status_callback(lib: &Library) {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_CONNECTION_STATUS_CALLBACK в библиотеке
        let connection_status_callback: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char) -> c_long> = lib.get(b"TRANS2QUIK_CONNECTION_STATUS_CALLBACK\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = connection_status_callback(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr()
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            8 => info!("TRANS2QUIK_QUIK_CONNECTED - соединение между терминалом QUIK и сервером установлено"),
            9 => {
                info!("TRANS2QUIK_QUIK_DISCONNECTED - соединение между терминалом QUIK и сервером разорвано");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            10 => info!(" TRANS2QUIK_DLL_CONNECTED - соединение между DLL и используемым терминалом QUIK установлено"),
            11 => {
                info!(" TRANS2QUIK_DLL_DISCONNECTED - соединение между DLL и используемым терминалом QUIK разорвано");
                info!("Error code: {}, error message: {}", error_code, error_message);
            }
            _ => info!("Unknown result code"),
        }
    }
}


/// Описание прототипа функции обратного вызова для обработки полученной информации о соединении
pub fn set_connection_status_callback(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию  TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK в библиотеке
        let set_connection_status_callback: Symbol<unsafe extern "C" fn(Option<TRANS2QUIK_CONNECTION_STATUS_CALLBACK>, *mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = set_connection_status_callback(
            Some(connection_status_callback(&lib)),
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            0 => info!("TRANS2QUIK_SUCCESS - функция обратного вызова установлена"),
            1 => {
                info!("TRANS2QUIK_FAILED - функцию обратного вызова установить не удалось");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            _ => info!("Unknown result code"),
        }

        if result == 10
        {
            return true
        }
        else
        {
            return false
        }
    }
}