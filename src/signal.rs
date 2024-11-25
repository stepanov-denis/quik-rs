use tracing::info;

/// Trading signal
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Signal {
    Buy,
    Sell,
}

/// The state of the EMA intersection
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum State {
    Above,   // The short EMA is higher than the long EMA
    Below,   // The short EMA is below the long EMA
    Between, // Within hysteresis (no clear signal)
}

/// Structure for generating trading signals with hysteresis
pub struct CrossoverSignal {
    hysteresis_percentage: f64,
    hysteresis_periods: usize,
    state: State,
    time_in_state: usize,
    last_signal: Option<Signal>,
}

impl CrossoverSignal {
    /// Creates a new CrossoverSignal structure with the given hysteresis parameters:
    /// hysteresis_percentage - the percentage threshold for the EMA difference to trigger a state change.
    /// hysteresis_periods - the number of consecutive periods the condition must be met before the signal is sent.
    pub fn new(hysteresis_percentage: f64, hysteresis_periods: usize) -> Self {
        Self {
            hysteresis_percentage,
            hysteresis_periods,
            state: State::Between,
            time_in_state: 0,
            last_signal: None,
        }
    }

    /// Updates the internal state with the latest short and long EMA values.
    /// Returns an optional Signal if a buy or sell signal was generated.
    pub fn update(&mut self, short_ema: f64, long_ema: f64) -> Option<Signal> {
        if long_ema == 0.0 {
            // Avoid division by zero
            return None;
        }

        let ema_diff = short_ema - long_ema;
        let ema_percentage = ema_diff / long_ema * 100.0;
        info!("start update signal");
        info!(
            "short_ema: {}, long_ema: {}, ema_percentage: {}, hysteresis_percentage: {}",
            short_ema, long_ema, ema_percentage, self.hysteresis_percentage
        );

        // Determine new state based on EMA percentage difference and hysteresis
        info!("check new state");
        let new_state = if ema_percentage > self.hysteresis_percentage {
            info!("state above");
            State::Above
        } else if ema_percentage < -self.hysteresis_percentage {
            info!("state below");
            State::Below
        } else {
            info!("state between");
            State::Between
        };

        info!("check change state");
        // Check if the state has changed
        if new_state == self.state {
            // The state has not changed, we increase the time in the state
            self.time_in_state += 1;
        } else {
            // State has changed, reset time in state
            self.state = new_state;
            self.time_in_state = 1;
        }

        // Check if the time in the current state exceeds the time hysteresis
        match self.state {
            State::Above => {
                if self.time_in_state >= self.hysteresis_periods
                    && self.last_signal != Some(Signal::Buy)
                {
                    self.last_signal = Some(Signal::Buy);
                    return Some(Signal::Buy);
                }
            }
            State::Below => {
                if self.time_in_state >= self.hysteresis_periods
                    && self.last_signal != Some(Signal::Sell)
                {
                    self.last_signal = Some(Signal::Sell);
                    return Some(Signal::Sell);
                }
            }
            State::Between => {
                // No signal in "Between" state
                self.time_in_state = 0;
            }
        }

        info!("no new signal");

        None
    }
}
