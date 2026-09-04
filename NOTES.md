# Actos Rust SDK — Mimari Notlar ve Geliştirme Günlüğü (NOTES.md)

Bu dosya, Actos Rust SDK (`actos`) geliştirme sürecinde alınan mimari kararları, backend uyumluluk notlarını ve sözleşme kurallarını kayıt altına alır.

---

## 1. Tip Sistemi ve `actos-types` Bağımlılığı

- **Doğrudan Crate Paylaşımı**: Tipler OpenAPI şemasından üretilmez; backend'in `actos-types` crate'i doğrudan bağımlılık olarak kullanılır (`../actos-backend/crates/actos-types`).
- **Derleme Zamanı Senkronizasyonu**: Backend API sözleşmesi veya DTO yapıları değiştiğinde SDK derleme aşamasında derhal yakalanır.
- **Sunucu Bağımlılıklarından Arındırılmışlık**: `actos-types` içinde `default-features = false` seçilerek `utoipa` ve sunucu tarafı bağımlılıklar devre dışı bırakılır. SDK yalnızca saf `serde` türetmelerini tüketir.
- **Git ve Yerel Bağımlılık**: Geliştirme ortamında `path = "../actos-backend/crates/actos-types"` kullanılır. Yayın veya bağımsız derlemelerde `[patch."https://github.com/actos-dev/backend"]` mekanizması devreye alınabilir.

---

## 2. Backend Faz 18.A Uyumu ve DEFERRED Özellikler (§0.3)

Canlı veya commit'li `GET /openapi.json` spesifikasyonu
(`../actos-backend/docs/openapi.json`) **otoritedir**: spec'te olmayan hiçbir uç
veya alan SDK'ya eklenmez.

Backend Faz 18.A tamamlanıp canlı spec'e girince aşağıdakiler ikinci bir
geçişle SDK'ya dahil edildi ve `PLAN.md`'de işaretlendi:

1. **Faz 12.B `inbox.*`**: `GET /me/inbox` (list/stream), `PATCH /me/inbox/{id}/read`,
   `POST /me/inbox/read` (read_all, opsiyonel `?cursor=`), yanıttaki `unread_count`
   üzerinden `unread_count()` ve sabit aralıkla `watch()` — `src/resources/inbox.rs`,
   `client.inbox()`, blocking cephe dahil, testler `tests/inbox_tests.rs`.
2. **`actors().update_me()` tri-state**: `display_name`/`bio`/`avatar` için
   `FieldUpdate::{Keep, Clear, Set}`; `.avatar(..)` yanında `.clear_display_name()`,
   `.clear_bio()`, `.clear_avatar()` — temizleme gövdede açık `null` gönderir.
3. **`feed().list()` / `feed().following()` `.actor_type(..)`** filtresi.
4. **`comments().list()` `.body_html(true)`** projeksiyonu (`?body_html=true`).

Geriye yalnızca **`verifications.*` (alan adı doğrulaması)** kalır — o da bu
yüzden SDK'ya eklenmez: backend `NOTES.md §9.2` gereği **v1 kapsamı dışına
DEFERRED** alınmıştır (beklemede değil, bilinçli kapsam dışı).

---

## 3. OpenAPI Spesifikasyon Referansı

- Canlı backend'i ayağa kaldırmadan spec doğrulaması yapmak için committed dosya referansı:
  `../actos-backend/docs/openapi.json`
- Canlı sunucu doğrulaması için (isteğe bağlı):
  `http://127.0.0.1:3100` (`docker compose up -d` && `cargo run -p actos-api`)

---

## 4. Temel SDK Sözleşmesi ve Güvenceler (§2)

1. **Tek Hata Tipi (`Error`)**:
   - `thiserror` tabanlı tek `enum Error`.
   - `Error::Api` varyantı `actos_types::ErrorCode`, `status`, `detail`, `request_id`, `retry_after`, `rate_limit` taşır.
   - `NotFound` (404) ile `Gone` (410) ayrımı açıkça korunur.
2. **Hız Limiti ve Kota Takibi**:
   - `X-RateLimit-*` başlıkları her HTTP yanıtında ayrıştırılır.
   - `RateLimit` durumu `Arc<Mutex<Option<RateLimit>>>` ile tüm istemci klonları arasında senkronize paylaşılır.
3. **Yeniden Deneme ve Idempotency**:
   - 429 yanıtlarında `Retry-After` süresine riayet edilir (varsayılan en fazla 2 yeniden deneme).
   - Güvenli olmayan POST işlemlerinde `Idempotency-Key` (UUIDv4) otomatik atanır; key bulunmayan istekler 5xx aldığında yeniden denenmez (§2.6).
4. **Sıfır Unsafe**:
   - Sandık kökünde koşulsuz `#![forbid(unsafe_code)]`.
