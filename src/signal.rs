use tracing::info;
/// Сигнал торговли
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Signal {
    Buy,  // Золотой крест - сигнал на покупку
    Sell, // Крест смерти - сигнал на продажу
}

/// Состояние пересечения EMA
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum State {
    Above,   // Короткая EMA выше длинной EMA
    Below,   // Короткая EMA ниже длинной EMA
    Between, // В пределах гистерезиса (нет четкого сигнала)
}

/// Структура для генерации торговых сигналов с гистерезисом
pub struct CrossoverSignal {
    hysteresis_percentage: f64,  // Гистерезис в процентах
    hysteresis_periods: usize,   // Гистерезис по времени (число периодов)
    state: State,                // Текущее состояние
    time_in_state: usize,        // Время в текущем состоянии
    last_signal: Option<Signal>, // Последний отправленный сигнал
}

impl CrossoverSignal {
    /// Создает новую структуру CrossoverSignal с заданными параметрами гистерезиса
    ///
    /// hysteresis_percentage - процентный порог для разницы EMA, чтобы вызвать изменение состояния.
    /// hysteresis_periods - количество последовательных периодов, в течение которых условие должно выполняться перед отправкой сигнала.
    pub fn new(hysteresis_percentage: f64, hysteresis_periods: usize) -> Self {
        Self {
            hysteresis_percentage,
            hysteresis_periods,
            state: State::Between,
            time_in_state: 0,
            last_signal: None,
        }
    }

    /// Обновляет внутреннее состояние с последними значениями короткой и длинной EMA.
    /// Возвращает опциональный Signal, если сгенерирован сигнал на покупку или продажу.
    pub fn update(&mut self, short_ema: f64, long_ema: f64) -> Option<Signal> {
        if long_ema == 0.0 {
            // Избежать деления на ноль
            return None;
        }

        let ema_diff = short_ema - long_ema;
        let ema_percentage = ema_diff / long_ema * 100.0;
        info!("Start update signal");
        info!("short_ema: {}, long_ema: {}, ema_percentage: {}, hysteresis_percentage: {}", short_ema, long_ema, ema_percentage, self.hysteresis_percentage);

        // Определяем новое состояние на основе процентной разницы EMA и гистерезиса
        info!("Check new state");
        let new_state = if ema_percentage > self.hysteresis_percentage {
            info!("State above");
            State::Above
        } else if ema_percentage < -self.hysteresis_percentage {
            info!("State below");
            State::Below
        } else {
            info!("State between");
            State::Between
        };

        info!("Check change state");
        // Проверяем, изменилось ли состояние
        if new_state == self.state {
            // Состояние не изменилось, увеличиваем время в состоянии
            self.time_in_state += 1;
        } else {
            // Состояние изменилось, сбрасываем время в состоянии
            self.state = new_state;
            self.time_in_state = 1;
        }

        // Проверяем, превышает ли время в текущем состоянии гистерезис по времени
        match self.state {
            State::Above => {
                if self.time_in_state >= self.hysteresis_periods {
                    if self.last_signal != Some(Signal::Buy) {
                        self.last_signal = Some(Signal::Buy);
                        return Some(Signal::Buy);
                    }
                }
            }
            State::Below => {
                if self.time_in_state >= self.hysteresis_periods {
                    if self.last_signal != Some(Signal::Sell) {
                        self.last_signal = Some(Signal::Sell);
                        return Some(Signal::Sell);
                    }
                }
            }
            State::Between => {
                // Нет сигнала в состоянии "Between"
                self.time_in_state = 0;
            }
        }

        info!("No new signal");

        None // Нет нового сигнала
    }
}
