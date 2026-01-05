# Деплой majowuji на archbook

## Сервер

| Параметр | Значение            |
|----------|---------------------|
| Host     | 192.168.0.10        |
| SSH Port | 2222                |
| User     | sergey              |
| SSH Key  | ~/.ssh/archbook_key |

## Структура на сервере

```
/home/sergey/majowuji/
├── majowuji           # бинарник
├── majowuji.db        # база данных
└── .env               # TELOXIDE_TOKEN
```

## Деплой

### 1. Сборка release

```bash
cd /home/ryazanov/Development/fitness/majowuji
cargo build --release
```

### 2. Копирование файлов

```bash
# Бинарник
ansible archbook -i ansible/inventory/hosts.yml \
  -m copy -a "src=target/release/majowuji dest=/home/sergey/majowuji/majowuji mode=0755"

# .env (только при первом деплое)
ansible archbook -i ansible/inventory/hosts.yml \
  -m copy -a "src=.env dest=/home/sergey/majowuji/.env mode=0600"
```

### 3. Systemd сервис

```bash
# Установить сервис (требует sudo)
ansible archbook -i ansible/inventory/hosts.yml \
  -m copy -a "src=/home/sergey/majowuji/majowuji-bot.service dest=/etc/systemd/system/majowuji-bot.service mode=0644 remote_src=yes" \
  --become

# Перезагрузить systemd
ansible archbook -i ansible/inventory/hosts.yml \
  -m systemd -a "daemon_reload=yes" --become

# Включить и запустить
ansible archbook -i ansible/inventory/hosts.yml \
  -m systemd -a "name=majowuji-bot enabled=yes state=started" --become
```

## Быстрый редеплой

После изменения кода:

```bash
cargo build --release && \
ansible archbook -i ansible/inventory/hosts.yml \
  -m copy -a "src=target/release/majowuji dest=/home/sergey/majowuji/majowuji mode=0755" && \
ansible archbook -i ansible/inventory/hosts.yml \
  -m systemd -a "name=majowuji-bot state=restarted" --become
```

## Управление сервисом

```bash
# Статус
ansible archbook -i ansible/inventory/hosts.yml \
  -m shell -a "systemctl status majowuji-bot"

# Логи
ansible archbook -i ansible/inventory/hosts.yml \
  -m shell -a "journalctl -u majowuji-bot -n 50 --no-pager"

# Рестарт
ansible archbook -i ansible/inventory/hosts.yml \
  -m systemd -a "name=majowuji-bot state=restarted" --become

# Стоп
ansible archbook -i ansible/inventory/hosts.yml \
  -m systemd -a "name=majowuji-bot state=stopped" --become
```

## База данных

```bash
# Просмотр записей
ansible archbook -i ansible/inventory/hosts.yml \
  -m shell -a "sqlite3 /home/sergey/majowuji/majowuji.db 'SELECT * FROM trainings ORDER BY date DESC LIMIT 10;'"

# Очистка (осторожно!)
ansible archbook -i ansible/inventory/hosts.yml \
  -m shell -a "sqlite3 /home/sergey/majowuji/majowuji.db 'DELETE FROM trainings; VACUUM;'"
```

## Файлы конфигурации

### ansible/inventory/hosts.yml

```yaml
all:
  hosts:
    archbook:
      ansible_host: 192.168.0.10
      ansible_port: 2222
      ansible_user: sergey
      ansible_ssh_private_key_file: ~/.ssh/archbook_key
      ansible_become_password: ****  # sudo password
```

### systemd/majowuji-bot.service

```ini
[Unit]
Description=Majowuji Telegram Bot
After=network.target

[Service]
Type=simple
User=sergey
WorkingDirectory=/home/sergey/majowuji
ExecStart=/home/sergey/majowuji/majowuji bot
Restart=always
RestartSec=5
Environment=RUST_LOG=info
EnvironmentFile=/home/sergey/majowuji/.env

[Install]
WantedBy=multi-user.target
```

## Troubleshooting

### TerminatedByOtherGetUpdates

Бот уже запущен где-то ещё (локально). Остановить локальный инстанс:
```bash
pkill -f "majowuji bot"
```

### Missing sudo password

Добавить `ansible_become_password` в inventory или использовать `-K`:
```bash
ansible archbook -i ... --become -K
```
