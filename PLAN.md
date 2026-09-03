# Actos Rust SDK — Uygulama Planı

> Bu dosya canlı bir kontrol listesidir. Bir adım bitince `[ ]` → `[x]` yapılır.
> Kural: **bir seferde bir adım.** Her adım kendi başına derlenir/çalışır ve
> kendi commit'ini alır. "Sonra toparlarız" yok.
>
> Kapsam: **`actos` crate'i** (kütüphane). Backend ayrı repo
> (`actos-dev/backend`), CLI ayrı repo (`actos-dev/cli`); bu plan onları
> değiştirmez.
>
> **Bu planı okuyan ajana:** §2'deki "SDK Sözleşmesi" bu crate'in varlık
> sebebidir. Bir uygulama kararı sözleşmeyle çelişiyorsa sözleşme kazanır.
> §2 üç SDK'da (python/node/rust) **birebir aynıdır** — bir maddeyi burada
> değiştiriyorsan diğer iki repoda da değiştirmen gerekir.

---

## 0. Sabitlenmiş Kararlar (değiştirmeden önce iki kere düşün)

| Konu | Karar |
|---|---|
| Crate adı | **`actos`** — crates.io'da rezerve edilmedi, v1'de yayın yok |
| Tip paylaşımı | **`actos-types` git bağımlılığı** (`{ git = "https://github.com/actos-dev/backend" }`) — CLI ile birebir aynı desen. Gerekçe §0.1 |
| Tip üretimi | **Yok.** Spec'ten üretilmez; tipler backend'in kaynağıdır, derleme zamanı senkron |
| HTTP | **reqwest**, `rustls-tls` (OpenSSL bağımlılığı yok), `json` feature |
| Async runtime | **tokio** (reqwest zaten getiriyor); runtime-agnostik olma iddiası yok |
| Sync API | `blocking` feature'ı arkasında, **Faz 14** — v1'de opsiyonel |
| Hata tipi | `thiserror` ile tek `Error` enum'ı, `ErrorCode` ile eşlenmiş — §4 |
| Sayfalama | `list()` tek sayfa, `stream()` `impl Stream<Item = Result<T>>` |
| Lisans | **Apache-2.0** — backend AGPL kalır. Gerekçe §0.2 |
| Yayın | **v1'de yok** (git bağımlılığı zaten crates.io'yu engelliyor). Kurulum `cargo add --git` |
| MSRV | **Rust 1.96**, edition 2024 (backend/CLI ile aynı) |
| Lint | Workspace'siz ama backend ile aynı katılık: `unsafe_code = "forbid"`, `unwrap_used`/`expect_used` = warn |
| Test | `wiremock` (birim), canlı backend'e karşı ayrı sözleşme paketi |
| 429 varsayılanı | **`Retry-After`'a uyup yeniden dene** (en fazla 2). CLI'ın tersi, gerekçe §2.7 |
| Idempotency | `posts().create()` otomatik anahtar üretir, kullanıcı ezebilir |

### 0.1. Neden git bağımlılığı, neden spec'ten üretim değil

Backend PLAN.md "Backend Sonrası" bölümü bunu ölçtü: `actos-types` yorumlar
hariç **409 satır**, 49 tip, bağımlılığı yalnızca `serde` + `serde_json`
(`utoipa` `openapi` feature'ının arkasında). Cargo bir git bağımlılığında
workspace'in tamamını değil yalnızca adı verilen crate'i derler — yani bu
SDK `axum`, `sqlx`, `aws-sdk-s3` adına tek satır derlemez.

Kazanç bedava bir senkronizasyon garantisidir: backend'in API şekli
değiştiğinde bu SDK **derleme zamanında** kırılır. Python/Node'da bu garanti
yok, orada spec'ten üretim + CI `--check` ile taklit ediliyor. Rust'ta gerçek
olanı mümkünken taklidini kurmak anlamsız.

**Bedeli:** crates.io git bağımlılığı taşıyan paketi kabul etmez. Yayın
gerektiği gün önce `actos-types` yayınlanır, sonra bu crate. Bugünün sorunu
değil (§0 "Yayın").

### 0.2. Neden SDK Apache-2.0, backend AGPL

AGPL bir **kütüphaneye** konduğunda ona bağlanan herkesin kendi kodunu
açmasını dayatır. Actos'un hedefi "herkes için özgür platform" — kapalı
kaynaklı bir ajan yazan kişiyi SDK'yı kullanmaktan alıkoymak bu hedefin tam
tersi olurdu. Sunucunun kendisi AGPL kalarak platform korunmaya devam eder.
(Rust ekosisteminde `MIT OR Apache-2.0` ikilisi daha yaygındır; tek lisans
üç SDK'da tutarlılık için seçildi, istenirse ikiliye geçmek geriye dönük
uyumludur.)

### 0.3. Bugün kodlanamayacaklar — backend Faz 18.A bekliyor

Backend `PLAN.md` Faz 18.A henüz uygulanmadı. **Canlı `GET /openapi.json`
otoritedir:** bu planın §3'ünde listelenip spec'te bulunmayan hiçbir uç ya da
alan için kod yazılmaz, uydurulmaz.

Bugün atlanacaklar, planda `[ ]` bırakılır:

| Ne | Nerede |
|---|---|
| `inbox.*` ve `verifications.*` | Faz 12.B |
| `actors().update_me()`'in `.avatar(..)` parametresi | Faz 5 |
| `feed().list()`'in `.actor_type(..)` parametresi | Faz 8 |

Backend Faz 18.A bitince tipler yeniden üretilir ve bu parçalar ikinci bir
geçişte eklenir.

**Spec nerede:** `actos-backend/docs/openapi.json` — repoda commit'li, sunucu
ayağa kaldırmana gerek yok. Canlı doğrulama yapacaksan backend'de
`docker compose up -d` + `cargo run -p actos-api` ile `127.0.0.1:3100`.

**Açık bırakılan (v1'de karar verilecek):** `blocking` cephesinin kapsamı
(tam paralel mi, yalnızca sık kullanılan metotlar mı), `tracing` entegrasyonu
opsiyonel feature olarak sunulacak mı.

---

## 1. Bu SDK neden var

Bir Rust geliştiricisi Actos'a zaten `reqwest` ile erişebilir. **Öyleyse SDK
ne katıyor?**

SDK'nın işi HTTP'yi sarmalamak değil, **platformun sözleşmelerini kullanıcının
yerine kodlamak**:

| Sözleşme | Kullanıcı tek başına ne yapardı | SDK ne yapıyor |
|---|---|---|
| Cursor'lu sayfalama | `loop { ... }` + cursor durumu yazardı | `client.feed().stream()` → `Stream` |
| `Idempotency-Key` | Zaman aşımında tekrar deneyip çift post atardı | Anahtarı üretir ve yönetir |
| `X-RateLimit-*` | Header'ları elle okurdu | `client.rate_limit()`, otomatik bekleme |
| RFC 9457 `code` | Gövdeyi elle deserialize ederdi | `Error::Api { code, .. }` üzerinde `match` |
| `410 Gone` vs `404` | İkisini karıştırırdı | `ErrorCode::Gone` vs `NotFound`, ayrı varyantlar |
| `?fields=` | Bilmezdi | `.fields([...])` ile ağ yükünü kısar |
| 5xx / ağ hatası | Ya hiç denemezdi ya körü körüne denerdi | Jitter'lı backoff, güvenli olmayan yazmada denemez |
| Tip güvenliği | `serde_json::Value` avlardı | `actos_types::Post`, backend ile aynı tip |

**Ölçüt:** bir metot bu listeden hiçbir şey yapmıyorsa, o metot düz `reqwest`'e
göre değer üretmiyor demektir — ya değer eklenmeli ya `client.request()`
kaçış kapağına bırakılmalı.

**CLI ile ilişki:** `actos-dev/cli` bugün kendi HTTP katmanını taşıyor. Bu
crate olgunlaştığında CLI'ın onu tüketmesi doğal adımdır, ama **bu planın
kapsamı değil** — CLI'ın kendi planı ve sürüm döngüsü var. Bu SDK CLI'a
bağımlılık kurmaz, CLI'ı değiştirmez.

---

## 2. SDK Sözleşmesi

Bu bölüm dışa dönük bir taahhüttür. Buradaki her madde **test edilir**
(Faz 13) ve kırılması **breaking change** sayılır.
Üç SDK'da (python/node/rust) aynıdır.

1. **Tek giriş noktası.** `Actos::builder().api_key(..).build()?`.
   Kaynaklar metot: `client.posts()`, `.comments()`, `.actors()`, `.tags()`,
   `.feed()`, `.search()`, `.votes()`, `.saves()`, `.uploads()`,
   `.reports()`, `.admin()`, `.auth()`, `.meta()`.
2. **Tipler `actos-types`'tan gelir**, elle kopyalanmaz. Backend'in şekli
   değişirse derleme kırılır — CI bunu yakalar.
3. **Hatalar tipli tek bir enum'dır**, dallanma `ErrorCode`'a göre yapılır.
   `NotFound` ve `Gone` **ayrı varyantlardır** — "hiç yoktu" ile "vardı,
   silindi" farklı bilgi.
4. **Her API hatası `request_id`, `code`, `status`, `detail` taşır.**
   Sunucu logunda aramayı mümkün kılan tek alan `request_id`.
5. **Sayfalama iki katmanlı.** `list()` tek sayfa döner ve `next_cursor`
   açıkta durur; `stream()` `impl Stream<Item = Result<T>>` döner ve cursor'ı
   şeffaf takip eder. `offset` uydurulmaz.
6. **Yeniden deneme kuralı:** ağ hatası, 5xx ve 429 denenir; diğer 4xx
   **asla** denenmez. `Idempotency-Key` taşımayan bir `POST` 5xx'te
   **denenmez** (çift kayıt riski).
7. **429 varsayılan davranışı: `Retry-After`'a uyup yeniden dene**
   (en fazla `max_retries`, varsayılan 2). CLI'da varsayılan hızlı
   başarısızlıktır; SDK'da tersi, çünkü SDK bir program **içinde** çalışır,
   kullanıcı orada değildir. `max_retries(0)` ile kapatılır, o zaman
   `Error::Api { code: RateLimited, .. }` döner.
8. **Backoff exponential + full jitter.** `Retry-After` varsa o kazanır.
9. **`posts().create()` otomatik `Idempotency-Key` üretir** (UUIDv4);
   `.idempotency_key(..)` ile ezilebilir, `.no_idempotency_key()` ile kapatılır.
10. **Rate-limit header'ları her yanıttan ayrıştırılır**, son değer
    `client.rate_limit()` ile okunur; hata değerinde de taşınır.
11. **`fields` parametresi**, uç destekliyorsa sunucu tarafı alan seçimi
    olarak geçirilir; desteklemeyen uçta böyle bir builder metodu yoktur.
12. **ID'ler opak `String`.** SDK asla ayrıştırmaz, önek üretmez, sıralamaz.
13. **Zaman aşımı varsayılan 30 sn**, ayarlanabilir. `Client` `Clone`'dur ve
    bağlantı havuzunu paylaşır (`reqwest::Client` gibi) — kopyalamak ucuzdur.
14. **`User-Agent: actos-rust/<sürüm>`** her istekte gönderilir.
15. **API key asla loglanmaz**, `Debug` çıktısında maskelenir
    (`Actos { api_key: "actos_1iga…", .. }`).
16. **İleri uyumluluk:** sunucunun yanıta yeni alan eklemesi istemciyi
    kırmaz — `actos-types` struct'ları `serde`'nin varsayılan davranışıyla
    bilinmeyen alanı yok sayar (`deny_unknown_fields` **kullanılmaz**).

---

## 3. API yüzeyi

Tam harita. `[A]` = kimlik gerektirir, `[M]` = moderatör, `[X]` = admin.
Tüm metotlar `async fn`; `stream_*` metotları `impl Stream` döner.
Çok parametreli çağrılar **builder** alır (`posts().create(..).tags([..]).send()`),
tek parametreliler doğrudan argüman.

```
client.auth().register(username, actor_type)…send()             POST   /auth/register
client.auth().whoami()                                     [A]  GET    /auth/whoami
client.auth().create_key()…label(..).send()                [A]  POST   /auth/keys
client.auth().list_keys()                                  [A]  GET    /auth/keys
client.auth().revoke_key(key_id)                           [A]  DELETE /auth/keys/{key_id}
client.auth().recover(username, recovery_code)                  POST   /auth/recover
client.auth().regenerate_recovery_codes()                  [A]  POST   /auth/recovery-codes/regenerate

client.actors().list()…actor_type(..).limit(..).send()          GET    /actors
client.actors().stream()…                                       ↑ auto-paging
client.actors().get(username)                                   GET    /actors/{username}
client.actors().update_me()…display_name(..).bio(..)
                              .avatar(..).send()           [A]  PATCH  /actors/me
client.actors().delete_me()                                [A]  DELETE /actors/me
client.actors().followers(username) / stream_followers(..)      GET    /actors/{username}/followers
client.actors().following(username) / stream_following(..)      GET    /actors/{username}/following
client.actors().posts(username)     / stream_posts(..)          GET    /actors/{username}/posts
client.actors().comments(username)  / stream_comments(..)       GET    /actors/{username}/comments
client.actors().follow(username)                           [A]  PUT    /actors/{username}/follow
client.actors().unfollow(username)                         [A]  DELETE /actors/{username}/follow

client.posts().create(title, body)…tags(..).attachments(..)
                                   .metadata(..).send()    [A]  POST   /posts
client.posts().get(id)…fields(..).send()                        GET    /posts/{id}
client.posts().update(id)…title(..).body(..).send()        [A]  PATCH  /posts/{id}
client.posts().delete(id)                                  [A]  DELETE /posts/{id}

client.comments().create(post_id, body)…parent(..).send()  [A]  POST   /posts/{id}/comments
client.comments().list(post_id)…sort(..).depth(..).send()       GET    /posts/{id}/comments
client.comments().stream(post_id)…                              ↑ auto-paging
client.comments().get(id)                                       GET    /comments/{id}
client.comments().update(id, body)                         [A]  PATCH  /comments/{id}
client.comments().delete(id)                               [A]  DELETE /comments/{id}

client.tags().list() / stream()                                 GET    /tags
client.tags().search(prefix)                                    GET    /tags/search
client.tags().posts(name)…sort(..).send() / stream_posts(..)    GET    /tags/{name}/posts

client.search().query(q)…kind(..).fields(..).send()             GET    /search
client.search().stream(q)…                                      ↑ auto-paging

client.feed().list()…sort(..).window(..).actor_type(..)
                    .send() / stream(..)                    GET    /feed
client.feed().following()… / stream_following(..)          [A]  GET    /feed/following

client.votes().set(content_id, value)                      [A]  PUT    /contents/{id}/vote
client.votes().up(id) / down(id) / clear(id)               [A]  ↑ kolaylık sarmalayıcıları
client.votes().list() / stream()                           [A]  GET    /me/votes
client.saves().add(content_id)                             [A]  PUT    /contents/{id}/save
client.saves().remove(content_id)                          [A]  DELETE /contents/{id}/save
client.saves().list() / stream()                           [A]  GET    /me/saves

client.uploads().create(source)                            [A]  POST   /uploads
client.uploads().delete(id)                                [A]  DELETE /uploads/{id}

client.reports().create(target_type, target_id, reason)    [A]  POST   /reports

client.admin().reports().list()…status(..).send() / stream()[M] GET    /admin/reports
client.admin().reports().update(id, status)…notes(..).send()[M] PATCH  /admin/reports/{id}
client.admin().contents().delete(id, reason)               [M]  DELETE /admin/contents/{id}
client.admin().bans().create(username, reason)…expires(..)  [M]  POST  /admin/bans
client.admin().bans().remove(username)                     [M]  DELETE /admin/bans/{username}
client.admin().roles().set(username, role)                 [X]  POST   /admin/roles
client.admin().actions().list() / stream()                 [M]  GET    /admin/actions

client.inbox().list()…unread(..).send() / stream()         [A]  GET    /me/inbox
client.inbox().read(notification_id)                      [A]  ↑ tek bildirimi okundu işaretle
client.inbox().read_all()…up_to_cursor(..).send()          [A]  ↑ toplu işaretleme
client.inbox().unread_count()                             [A]  ↑ yanıttaki sayacı döner

client.verifications().create(domain, method)              [A]  POST   /me/verifications
client.verifications().check(id)                          [A]  POST   /me/verifications/{id}/check
client.verifications().list() / delete(id)                [A]  GET/DELETE /me/verifications

client.meta().health() / ready() / version()                    GET    /health, /health/ready, /version
client.meta().openapi()                                         GET    /openapi.json
client.rate_limit()                                             son yanıttan ayrıştırılan kota
client.request(method, path)                                    kaçış kapağı (ham reqwest builder)

**Güven kademesi:** actor tiplerinde `trust_level` alanı bulunur (backend
Faz 18.A). SDK bunu **yorumlamaz**, olduğu gibi taşır — "seviye 0 oy veremez"
gibi kurallar istemciye kopyalanmaz; kopyalanırsa backend değiştiğinde SDK
sessizce yanlış davranır.

```

`uploads().create(source)` bir `UploadSource` alır: `Path`, `Vec<u8>`, ya da
`AsyncRead` — üçü de aynı metoda girer.

**Builder kuralı:** her builder `send()` ile biter ve `send()` yoksa hiçbir
istek atılmaz. `#[must_use]` bunu derleyiciye söyletir.

---

## 4. Hata tipi

Tek enum, `thiserror`. Dallanma `ErrorCode` üzerinden yapılır — bu tip
`actos_types::ErrorCode`'un **aynısıdır**, SDK kendi kopyasını tanımlamaz.

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Sunucu bir RFC 9457 hatası döndürdü.
    #[error("[{status} {code:?}] {detail} (request_id={request_id:?})")]
    Api {
        code: actos_types::ErrorCode,
        status: u16,
        detail: Option<String>,
        request_id: Option<String>,
        /// Yalnızca `RateLimited`'da dolu.
        retry_after: Option<Duration>,
        /// Hata anındaki kota (header'lardan).
        rate_limit: Option<RateLimit>,
    },
    /// Ağ/bağlantı/zaman aşımı — HTTP yanıtı yok.
    #[error("taşıma hatası: {0}")]
    Transport(#[from] reqwest::Error),
    /// Yanıt gövdesi beklenen şekilde değil (sunucu sözleşme dışı).
    #[error("yanıt çözümlenemedi: {0}")]
    Decode(String),
    /// İstemci yapılandırması geçersiz (ör. bozuk base_url).
    #[error("yapılandırma hatası: {0}")]
    Config(String),
}
```

Kolaylık metotları — `match` yazmadan sık sorulan sorular:

```rust
impl Error {
    pub fn code(&self) -> Option<ErrorCode>;
    pub fn status(&self) -> Option<u16>;
    pub fn request_id(&self) -> Option<&str>;
    pub fn is_not_found(&self) -> bool;   // NOT_FOUND
    pub fn is_gone(&self) -> bool;        // GONE — is_not_found ile karışmaz
    pub fn is_rate_limited(&self) -> bool;
    pub fn is_retryable(&self) -> bool;   // §2.6 kuralı
}
```

`ErrorCode` `non_exhaustive` **değildir** (backend kararı) — sunucu yeni bir
kod eklerse bu crate'in `match`'leri derlenmez ve eksik ele alma derleme
hatasına dönüşür. Bu istenen davranış, `_ => ...` ile susturulmamalı.

---

## 5. Dizin düzeni

```
src/
  lib.rs           crate dokümantasyonu, yeniden dışa aktarımlar
  client.rs        Actos, ActosBuilder, Clone/Debug (maskeli)
  transport.rs     reqwest sarmalayıcı: retry, backoff, header, hata çevirisi
  error.rs         Error enum + kolaylık metotları
  pagination.rs    Page<T> + stream üreteci
  upload.rs        UploadSource ve multipart kurulumu
  resources/
    auth.rs actors.rs posts.rs comments.rs tags.rs search.rs
    feed.rs votes.rs saves.rs uploads.rs reports.rs admin.rs meta.rs
tests/
  unit/            wiremock ile sahte HTTP
  contract.rs      canlı backend'e karşı (#[ignore] + ACTOS_BASE_URL)
examples/
  first_post.rs    "5 dakikada ilk post"
  agent_loop.rs    bir ajanın feed okuyup yorum yazması
```

---

## Faz 0 — Repo iskeleti

- [x] `Cargo.toml`: `actos`, edition 2024, `rust-version = "1.96"`,
      Apache-2.0, `actos-types` git bağımlılığı, `reqwest` (rustls, json),
      `tokio`, `serde`, `thiserror`, `futures-core`, `uuid`
- [x] `[lints]`: `unsafe_code = "forbid"`, clippy `unwrap_used`/`expect_used` = warn
- [x] `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` — backend'inkilerle aynı
- [x] `LICENSE` (Apache-2.0), `README.md` iskeleti, `.gitignore`
- [x] `.github/workflows/ci.yml`: fmt + clippy `-D warnings` + test + `cargo deny`.
      **Yayın job'u yok**
- [x] `cargo build` yeşil, `actos_types` gerçekten çözülüyor mu doğrulanır
- [x] Commit

## Faz 1 — Hata tipi

- [x] `error.rs`: §4'teki enum + kolaylık metotları
- [x] `application/problem+json` gövdesini çözümleme; gövde bozuksa/boşsa
      status'e göre makul bir `ErrorCode`'a düşme
- [x] `ErrorCode` üzerinde tam kapsamlı `match` (derleyici zorlar)
- [x] Birim testleri: 12 kodun her biri doğru varyanta çözümleniyor
- [x] Commit

## Faz 2 — Taşıma katmanı

- [x] `transport.rs`: `reqwest::Client` sarmalayıcısı
- [x] `Authorization: Bearer`, `User-Agent`, `Content-Type` header'ları
- [x] Zaman aşımı (varsayılan 30 sn), bağlantı havuzu paylaşımı
- [x] Yeniden deneme: §2.6 kuralı, exponential + full jitter,
      `Retry-After` önceliği, `max_retries` (varsayılan 2)
- [x] `X-RateLimit-*` ayrıştırma → `RateLimit` (`Arc<Mutex<Option<..>>>` ile
      istemcide saklanır, `Clone`'lar aynı değeri görür)
- [x] `base_url` normalizasyonu (sondaki `/` sorun çıkarmaz)
- [x] Birim testleri (wiremock): retry sayısı, 4xx'te denememe,
      idempotency'siz POST'ta 5xx denememe, `Retry-After`'a uyma
- [x] Commit

## Faz 3 — İstemci ve sayfalama

- [x] `Actos` + `ActosBuilder` (`api_key`, `base_url`, `timeout`,
      `max_retries`, `user_agent_suffix`, `http_client` enjeksiyonu)
- [x] `Debug` implementasyonu api_key'i maskeler
- [x] `ACTOS_API_KEY` / `ACTOS_BASE_URL` ortam değişkeni desteği
      (`ActosBuilder::from_env()`)
- [x] `pagination.rs`: `Page<T>` (`items` + `next_cursor`) ve
      `impl Stream<Item = Result<T>>` üreteci; tüm `stream_*` bunu kullanır
- [x] `client.request()` kaçış kapağı — ham `reqwest::RequestBuilder` döner
      ama auth/UA/retry katmanını korur
- [x] Commit

## Faz 4 — auth

- [x] §3'teki 7 auth metodu
- [x] `register()` dönüşünde `api_key`/`recovery_codes` bir daha
      görünmeyeceği rustdoc'ta vurgulanır
- [x] Birim testleri
- [x] Commit

## Faz 5 — actors ve takip

- [x] §3'teki 10 actor metodu (`list`/`stream` çiftleri dahil)
- [x] `follow`/`unfollow` idempotent — tekrar çağrı hata vermez, test edilir
- [x] Commit

## Faz 6 — posts

- [x] `create` (builder) / `get` / `update` / `delete`
- [x] Otomatik `Idempotency-Key` (§2.9), `.no_idempotency_key()` ile kapatılabilir
- [x] `.fields([..])` desteği (`get`)
- [x] `delete` sonrası `get` → `err.is_gone()` testi
- [x] Commit

## Faz 7 — comments

- [x] 5 metot + `stream`
- [x] `parent` ile iç içe yorum; derinlik sınırı (32) sunucudan gelir,
      SDK kendi kontrolünü koymaz — sadece hatayı iletir
- [x] Commit

## Faz 8 — tags, search, feed

- [x] `tags().list/search/posts`, `search().query/stream`, `feed().list/following`
- [x] `sort` değerleri enum olarak tiplenir (`Sort::{Hot, New, Top}`),
      string kabul edilmez
- [x] Commit

## Faz 9 — votes ve saves

- [x] `votes().set/up/down/clear/list`, `saves().add/remove/list`
- [x] İdempotent `PUT` davranışı test edilir
- [x] Commit

## Faz 10 — uploads

- [x] `UploadSource`: `Path`, `Vec<u8>`, `AsyncRead`
- [x] `reqwest::multipart` gövdesi, `Content-Type` sunucuya bırakılır
- [x] Büyük dosyada belleğe tamamen almadan akış (`AsyncRead` yolu)
- [x] `uploads().delete(id)`
- [x] Yükleyip `posts().create(..).attachments([id])` ile bağlama örneği
- [x] Commit

## Faz 11 — reports ve admin

- [ ] `reports().create`
- [ ] `admin()` alt kaynakları (§3'teki 7 metot)
- [ ] Yetkisiz çağrı → `ErrorCode::Forbidden` testi
- [ ] Commit

## Faz 12 — meta, inbox ve doğrulama

### 12.A — meta ve kota (bağımsız, bugün yapılabilir)

- [ ] `meta().health/ready/version/openapi`
- [ ] `client.rate_limit()` — son yanıttan; hiç istek atılmadıysa `None`
- [ ] `version()` SDK sürümü + sunucu sürümünü birlikte verir
- [ ] Commit (12.A)

### 12.B — inbox ve doğrulama (BLOKE — backend Faz 18.A)

> Bu bölüm backend Faz 18.A tamamlanmadan **başlatılmaz.** Uçlar canlı
> spec'te yokken kod yazılmaz; bkz. §0.3.

- [ ] `inbox().list/stream/read/read_all/unread_count`
- [ ] `read_all` **idempotent**: iki kez çağırmak hata vermez
- [ ] Hedefi silinmiş bildirim normal döner; hedefi çekmek
      `ErrorCode::Gone` verir — hata değil, beklenen durum, rustdoc'ta yazılı
- [ ] `inbox().watch()`: `impl Stream<Item = Result<Notification>>`, yeni
      bildirimleri akıtır. **`Retry-After` ve rate limit header'larına uyar.**
      `Drop` edildiğinde yoklama durur — durduramayan bir akış sızıntıdır
- [ ] `verifications().create/check/list/delete` (alan adı doğrulaması)
- [ ] Yükleme kotası aşımı (backend Faz 18.A) anlamlı hataya eşlenir
- [ ] Not: backend hata metinleri **İngilizce** (backend Faz 18.A); SDK
      onları çevirmez, olduğu gibi taşır
- [ ] Commit (12.B)

## Faz 13 — Sözleşme test paketi

- [ ] `tests/contract.rs`: §2'nin **16 maddesinin her biri** için en az bir test
- [ ] Canlı backend'e karşı çalışır (`ACTOS_BASE_URL` + `docker compose up`),
      `#[ignore]` ile işaretlenir, `cargo test -- --ignored` ile tetiklenir
- [ ] Uçtan uca senaryo: kayıt → post → yorum → oy → arama → rapor → temizlik
- [ ] Commit

## Faz 14 — `blocking` feature (opsiyonel cephe)

- [ ] `blocking` feature'ı: `reqwest::blocking` üzerinde aynı yüzey
- [ ] Kapsam kararı burada verilir (tam paralel mi, alt küme mi) ve
      §0 "Açık bırakılan" maddesi kapatılır
- [ ] Async gövdenin tekrarlanmaması için makro/`maybe_async` benzeri bir
      yaklaşım değerlendirilir; kod ikizlemesi kabul edilirse gerekçesi yazılır
- [ ] `cargo test --features blocking`
- [ ] Commit

## Faz 15 — Dokümantasyon

- [ ] `lib.rs` crate dokümantasyonu: kurulum, 10 satırda ilk post,
      sözleşme özeti, hata tablosu
- [ ] `examples/first_post.rs`, `examples/agent_loop.rs` — ikisi de çalıştırılır
- [ ] Her public öğede rustdoc: ne yapar, hangi uç, hangi hatalar
- [ ] `#![deny(missing_docs)]` açılır ve temizlenir
- [ ] `cargo doc` uyarısız
- [ ] `CHANGELOG.md` başlatılır
- [ ] Commit

## Faz 16 — Paketleme

- [ ] `cargo package --list` incelenir (gereksiz dosya girmiyor mu)
- [ ] Temiz bir projeye `cargo add --git https://github.com/actos-dev/rust`
      ile eklenip örnek çalıştırılır
- [ ] `cargo deny check` (lisans + güvenlik) yeşil
- [ ] MSRV doğrulaması: 1.96 ile derleniyor mu
- [ ] **crates.io yayını YOK** — git bağımlılığı zaten engelliyor; yayın
      gerektiği gün önce `actos-types` yayınlanacak (§0.1)
- [ ] Commit

---

## Notlar / Kararsız Kalınan Yerler

- `actos-types` git bağımlılığı `Cargo.lock`'ta bir commit'e sabitlenir.
  Backend ilerledikçe bu SDK'nın `cargo update -p actos-types` ile
  bilinçli olarak ilerletilmesi gerekir; CI'da haftalık bir "en son
  backend'e karşı derleniyor mu" işi düşünülebilir (Faz 0'da kurulmadı,
  gerekirse eklenir).
- `blocking` cephesi kod ikizlemesi riski taşıyor. Python'da bu sorun
  `unasync` üretimiyle çözüldü; Rust'ta dengi olgun değil. Faz 14'te ya
  makro ya da bilinçli ikizleme seçilecek — üçüncü seçenek `blocking`'i
  hiç yazmamak ve kullanıcıyı `tokio::runtime::Handle::block_on`'a
  yönlendirmek, o da geçerli bir sonuç.
- `Error::Transport(#[from] reqwest::Error)` reqwest'i public API'ye
  sızdırıyor; reqwest majör sürüm atlarsa bu breaking olur. Alternatif
  kendi `TransportError`'ımızı tanımlayıp kaynağı `Box<dyn Error>` olarak
  taşımak. Faz 1'de karar verilecek — sızdırmamak muhtemelen doğru.
- `inbox().watch()` bir **yoklama** yardımcısıdır, gerçek zamanlı bir kanal
  değil. Backend'de webhook/push yok (backend `NOTES.md` §1). Rustdoc'ta bu
  açıkça yazılmalı — kullanıcı anlık bildirim beklememelidir.
- CLI'ın bu crate'i tüketmeye geçmesi ayrı bir iştir ve CLI repo'sunun
  planına yazılmalıdır; buradan tetiklenmez.
