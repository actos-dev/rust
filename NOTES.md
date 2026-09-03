# Actos Rust SDK — Mimari Notlar ve Geliştirme Günlüğü (NOTES.md)

Bu dosya, Actos Rust SDK (`actos`) geliştirme sürecinde alınan mimari kararları, backend uyumluluk notlarını ve sözleşme kurallarını kayıt altına alır.

---

## 1. Tip Sistemi ve `actos-types` Bağımlılığı

- **Doğrudan Crate Paylaşımı**: Tipler OpenAPI şemasından üretilmez; backend'in `actos-types` crate'i doğrudan bağımlılık olarak kullanılır (`../actos-backend/crates/actos-types`).
- **Derleme Zamanı Senkronizasyonu**: Backend API sözleşmesi veya DTO yapıları değiştiğinde SDK derleme aşamasında derhal yakalanır.
- **Sunucu Bağımlılıklarından Arındırılmışlık**: `actos-types` içinde `default-features = false` seçilerek `utoipa` ve sunucu tarafı bağımlılıklar devre dışı bırakılır. SDK yalnızca saf `serde` türetmelerini tüketir.
- **Git ve Yerel Bağımlılık**: Geliştirme ortamında `path = "../actos-backend/crates/actos-types"` kullanılır. Yayın veya bağımsız derlemelerde `[patch."https://github.com/actos-dev/backend"]` mekanizması devreye alınabilir.

---

## 2. Backend Faz 18.A Bekleyen ve Ertelenen Özellikler (§0.3)

Backend `PLAN.md` Faz 18.A henüz tamamlanmadığından, canlı veya commit'li `GET /openapi.json` spesifikasyonunda (`../actos-backend/docs/openapi.json`) yer almayan hiçbir uç veya alan SDK'ya eklenmez:

1. **Faz 12.B (`inbox.*` ve `verifications.*`)**:
   - `GET /me/inbox`, `POST /me/verifications` vb. uçlar ertelenmiştir.
   - Bu faz tamamlanana kadar SDK yüzeyine eklenmeyecek, `PLAN.md` üzerinde `[ ]` olarak bırakılacaktır.
2. **`actors().update_me()`**:
   - Mevcut backend `UpdateProfileRequest` yalnızca `display_name` ve `bio` alanlarını kabul eder.
   - Taslaklarda geçen `.avatar(..)` parametresi SDK builder'ına dahil edilmemiştir.
3. **`feed().list()`**:
   - Mevcut backend `GET /feed` ucu `actor_type` sorgu parametresini desteklemez.
   - Builder üzerinde `.actor_type(..)` filtresi eklenmemiştir.

Backend Faz 18.A tamamlandığında bu uçlar ve parametreler ikinci bir geçişle SDK'ya dahil edilecektir.

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
