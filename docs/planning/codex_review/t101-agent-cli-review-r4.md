# Ревью T-101, раунд r4

## Итог

Патч пока нельзя считать полностью корректным: обычные сценарии r3 исправлены, но новый `Database::open_existing` по-прежнему открывает SQLite с флагом создания. Поэтому проверка существования пути остаётся TOCTOU-проверкой, а обещание «existing/no-create» не обеспечено самим SQLite-open.

## Finding

### [P2] Открывать проверенные пути без `SQLITE_OPEN_CREATE`

`src/db/mod.rs:68`: если существующий путь удалить или подменить между `Path::exists()` в `main` и этим вызовом (либо вызвать публичный `open_existing` для отсутствующего пути напрямую), `Connection::open` использует `READ_WRITE | CREATE`: будет создан пустой файл, после чего метод вернёт ошибку об отсутствии `trainings`. Та же гонка есть у `migrate`: после `metadata` он вызывает create-capable `Database::open` и при исчезновении файла может успешно создать новую текущую БД вместо обновления существующей. Это нарушает правило «No command creates the file silently» и existing-only смысл миграции ([AGENTS.md:27-34](/home/ryazanov/Development/fitness/majowuji/AGENTS.md#L27-L34)). Воспроизведение для метода не требует гонки: `Database::open_existing(<missing path>)` возвращает ошибку, но оставляет новый пустой файл; CLI-воспроизведение — удалить/переименовать файл между предварительной проверкой и SQLite-open. Нужен `Connection::open_with_flags(..., SQLITE_OPEN_READ_WRITE)` без `CREATE`; для `migrate` — такой же existing-only open с последующим явным `init_schema`.

## Перепроверка findings r3

1. **Существующий пустой/чужой файл без `--create` — основной дефект исправлен.** `log` и `intervals sync` без `--create` идут через `Database::open_existing`; метод проверяет `trainings` и текущую схему без `init_schema`. Тест `write_commands_do_not_initialize_existing_file_without_create` проверяет пустой файл, побайтовую неизменность foreign SQLite и отказ sync. Замена ветки обратно на `Database::open` статически уничтожает этот negative control. Осталась только отдельная TOCTOU-проблема выше.
2. **Дата cross-user строки — исправлена.** В `tests/cli.rs:421-430` она равна `start_dt + 45s`, то есть всегда находится внутри сессии после сэмпла `40s`. При снятии фильтра `user_id.is_none()` чужая строка гарантированно получает link: ожидание `training_id`, длина `filled` либо финальная проверка нулевого pulse становятся красными независимо от границы секунд.

Исторические утверждения о фактических mutant-прогонах независимо не подтверждаются: в дереве нет отдельных mutant patch/log/commit. Чувствительность обоих текущих negative control подтверждается статической трассировкой.

## Перепроверка цепочки r1–r2

- Частичная pulse-миграция закрыта: `pulse_before` и `pulse_after` проверяются и добавляются независимо, ошибка `ALTER TABLE` распространяется; CLI-тест сохраняет существующий `pulse_before` и требует появления `pulse_after`.
- Не-watch source отбрасывается до streams-запроса; сравнение с `ZEPP` точное и case-insensitive, а `STRAVA` и `ZEPPELIN` покрыты негативными тестами.
- CLI-idempotence harness выполняет два sync, проверяет одну строку `watch_sessions`, пустой второй `filled` и неизменный pulse.
- Пустое имя проверяется на уже валидной БД и требует конкретную диагностику.
- Cross-user product-path фильтрует `user_id IS NULL` до `link_pulses`; детерминированный negative control теперь полноценный.
- API-base override доступен только в debug-сборках; release использует константный HTTPS endpoint.
- Темпозависимость ожидаемого peak устранена: последний HR-сэмпл в CLI-фикстуре также равен `100`.

## Новая поверхность r4

- `open_existing` и `open_readonly` сейчас выполняют одинаковые проверки `trainings` и `schema_is_current`; функционального расхождения проверок нет. Различаются ожидаемо режим открытия и подсказка `--create`. Дублирование само по себе не поднято как finding.
- Обычный `migrate` отсутствующего или нулевого файла теперь отклоняется до `Database::open` (`src/main.rs:150-155`), что соответствует existing-only контракту; `migrate_requires_existing_database_and_rejects_json` покрывает оба случая. Неполнота остаётся только при гонке, описанной в finding.
- Явный `migrate` существующего непустого foreign SQLite добавляет схему majowuji. В заявленном контракте foreign-file запрет сформулирован для `log`/`intervals sync` без `--create`, а `migrate` является явной изменяющей командой, поэтому отдельно это не квалифицировано как дефект.

## Проверенные утверждения и ограничения

- HEAD — `f1a9845`, ветка `feat/agent-cli-db-json`; tracked working tree изменяет `src/db/mod.rs`, `src/intervals.rs`, `src/main.rs`, `tests/cli.rs`, плюс untracked `docs/planning/`.
- `git diff bf587c8` содержит 1772 строки и SHA-256 `2ad22f769ef27cd065c6c12ef48f3a87b22601f6c7f751a9fe6fbaab95940741`; он побайтно совпадает с `/home/ryazanov/.cache/tmp-codex-review/majowuji-branch.diff`.
- В diff ровно восемь заявленных файлов: `AGENTS.md`, `Cargo.toml`, `Cargo.lock`, `src/db/mod.rs`, `src/intervals.rs`, `src/lib.rs`, `src/main.rs`, `tests/cli.rs`.
- Статически насчитывается 160 test-атрибутов под `src/` и 21 тест в `tests/cli.rs`, что совпадает с заявленными числами.
- Заявленный зелёный прогон `160 lib + 21 cli` не повторялся: финальный write contract разрешает изменить только этот отчёт, тогда как Cargo и тесты создают/обновляют артефакты и временные БД. Поэтому факт прогона текущего snapshot остаётся неподтверждаемым из первичных логов.
- Живой Intervals.icu, production DB и комментарий о наблюдавшемся production source `ZEPP` не проверялись; `.env` не читался.

ВЕРДИКТ: FIXES
