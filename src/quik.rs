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
    DllAlreadyConnectedToQuik,
}

#[derive(Debug)]
pub enum Trans2quikError {
    TerminalNotFound { error_code: i32, error_message: String },
    DllVersionNotSupported { error_code: i32, error_message: String },
    Failed { error_code: i32, error_message: String },
    Unknown { error_code: i32, error_message: String },
}


impl fmt::Display for Trans2quikError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trans2quikError::TerminalNotFound { error_code, error_message } => {
                write!(f, "QUIK terminal not found. Error code: {}, Error message: {}", error_code, error_message)
            },
            Trans2quikError::DllVersionNotSupported { error_code, error_message } => {
                write!(f, "The version of Trans2QUIK.dll used is not supported. Error code: {}, Error message: {}", error_code, error_message)
            },
            Trans2quikError::Failed { error_code, error_message } => {
                write!(f, "An error occurred while establishing a connection. Error code: {}, Error message: {}", error_code, error_message)
            },
            Trans2quikError::Unknown { error_code, error_message } => {
                write!(f, "An unknown error occurred. Error code: {}, Error message: {}", error_code, error_message)
            },
        }
    }
}


impl From<libloading::Error> for Trans2quikError {
    fn from(err: libloading::Error) -> Trans2quikError {
        Trans2quikError::Failed {
            error_code: -1,
            error_message: err.to_string(),
        }
    }
}


impl std::error::Error for Trans2quikError {}


pub struct Terminal {
    library: Library,
}


impl Terminal {
    /// Функция используется для загрузки библиотеки DLL
    pub fn new(path: &str) -> Result<Self, libloading::Error> {
        unsafe {
            let library = Library::new(path)?;
            Ok(Terminal { library })
        }
    }


    /// Функция используется для установления связи с терминалом QUIK.
    pub fn trans2quik_connect(&self, connection_string: *const c_char, error_code: *mut c_long, error_message: *mut c_char, error_message_len: c_ulong) -> Result<c_long, libloading::Error> {
        // Определяем тип функции
        unsafe {
            // Найдем функцию TRANS2QUIK_CONNECT в библиотеке
            let connect: Symbol<unsafe extern "C" fn(*const c_char, *mut c_long, *mut c_char, c_ulong) -> c_long> = self.library.get(b"TRANS2QUIK_CONNECT\0")?;

            Ok(connect(connection_string, error_code, error_message, error_message_len))
        }
    }


    pub fn connect(&self) -> Result<Trans2quikResult, Trans2quikError> {
            // Вызываем функцию
            let connection_string = CString::new(r"c:\QUIK Junior").expect("CString::new failed");
            let mut error_code: c_long = 0;
            let mut error_message = vec![0 as c_char; 256];
            let error_message_len = error_message.len();

            let result = self.trans2quik_connect(
                connection_string.as_ptr(),
                &mut error_code as *mut c_long,
                error_message.as_mut_ptr(),
                error_message_len as c_ulong,
            )?;

            let error_message = unsafe {
                CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned()
            };

            match result {
                0 => {
                    info!("TRANS2QUIK_SUCCESS - соединение установлено успешно");
                    Ok(Trans2quikResult::Success)
                },
                2 => {
                    error!("TRANS2QUIK_QUIK_TERMINAL_NOT_FOUND");
                    Err(Trans2quikError::TerminalNotFound { error_code: error_code as i32, error_message })
                },
                3 => {
                    error!("TRANS2QUIK_DLL_VERSION_NOT_SUPPORTED");
                    Err(Trans2quikError::DllVersionNotSupported { error_code: error_code as i32, error_message })
                },
                4 => {
                    info!("TRANS2QUIK_DLL_ALREADY_CONNECTED_TO_QUIK");
                    Ok(Trans2quikResult::DllAlreadyConnectedToQuik)
                },
                1 => {
                    error!("TRANS2QUIK_FAILED - произошла ошибка при установлении соединения");
                    Err(Trans2quikError::Failed { error_code: error_code as i32, error_message })
                },
                _ => {
                    error!("Unknown result code");
                    Err(Trans2quikError::Unknown { error_code: error_code as i32, error_message })
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