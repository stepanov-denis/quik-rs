use std::collections::HashSet;
use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::ChatId;
use tokio::sync::Mutex;

/// Тип для списка подписчиков.
pub type Subscribers = Arc<Mutex<HashSet<ChatId>>>;

pub struct TgBot {
    pub bot: Bot,
    pub subscribers: Subscribers,
}

impl TgBot {
    /// Создаёт новый экземпляр TgBot с заданным токеном.
    pub fn new(token: &str) -> Self {
        TgBot {
            bot: Bot::new(token),
            subscribers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Асинхронно отправляет сообщение в указанный чат.
    async fn send_message(
        &self,
        chat_id: ChatId,
        text: &str,
    ) -> Result<(), teloxide::RequestError> {
        self.bot.send_message(chat_id, text).await?;
        Ok(())
    }

    /// Отправляет заданное сообщение всем подписчикам (по вызову метода, не циклично).
    pub async fn broadcast(&self, message: &str) {
        // Делаем снимок (snapshot) списка подписчиков.
        let subs_snapshot = {
            let subs = self.subscribers.lock().await;
            subs.clone()
        };

        for chat_id in subs_snapshot {
            if let Err(err) = self.send_message(chat_id, message).await {
                eprintln!("Ошибка при отправке сообщения чату {}: {}", chat_id, err);
            }
        }
    }

    /// Пример обновления списка подписчиков (например, вывод статистики).
    /// Здесь можно добавить дополнительную логику обновления, если требуется.
    pub async fn update_subscribers(&self) {
        let subs = self.subscribers.lock().await;
        println!("[Updater] Текущее количество подписчиков: {}", subs.len());
    }

    /// Запускает прослушивание входящих сообщений от Telegram.
    /// При получении команды "/start" chat_id пользователя добавляется в список подписчиков.
    /// Этот метод запускается в отдельной задаче.
    pub async fn start_message_listener(&self) {
        let bot = self.bot.clone();
        let subscribers = self.subscribers.clone();
        tokio::spawn(async move {
            teloxide::repl(bot, move |message: Message, bot: Bot| {
                let subscribers = subscribers.clone();
                async move {
                    if let Some(text) = message.text() {
                        if text == "/start" {
                            {
                                let mut subs = subscribers.lock().await;
                                subs.insert(message.chat.id);
                            }
                            if let Err(err) =
                                bot.send_message(message.chat.id, "Вы подписались на рассылку!").await
                            {
                                eprintln!("Ошибка отправки сообщения: {}", err);
                            }
                        }
                    }
                    respond(())
                }
            })
            .await;
        });
    }
}