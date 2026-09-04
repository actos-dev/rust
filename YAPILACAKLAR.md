# Yapılacaklar — Rust SDK

> **GÜNCELLEME (2026-09-05):** publish dışı tüm 18.A eksikleri tamamlandı — inbox.*,
> feed actor_type, tri-state `FieldUpdate` update_me (avatar/clear), comments body_html,
> `[silindi]`→`[deleted]`, İngilizce hata mesajları, CI `cargo deny`. Kapılar yeşil
> (cargo test --all-features, clippy -D warnings, fmt). Kalan yalnızca paketleme/yayın
> (`actos-types` path→git bağımlılığı).

> Durum özeti: Faz 0–16 kodu derleniyor, `cargo test --all-features` (95 test +
> 7 doctest) yeşil, `cargo clippy -D warnings` ve `cargo deny check` temiz.
> Ancak backend Faz 18.A'nın 2026-09-03'te bitmesiyle SDK'nın yüzeyi geride
> kaldı: `/me/inbox`, `feed`'de `actor_type`, `update_me()`'de `.avatar(..)`,
> ve yorum ağacında `body_html` bayrağı canlı spec'te var ama SDK'da yok.
> Ayrıca paketleme **fiilen kırık**: `actos-types` bir `path` bağımlılığı,
> `cargo package` bunu somut olarak reddediyor. `verifications.*` planda
> `[ ]` duruyor ama backend'de süresiz ertelendi — bu madde plandan
> düşülmeli, "yapılacak" değil.
> Son kontrol: 2026-09-03, backend Faz 18.A sonrası, `actos-backend`
> `docs/openapi.json` (2026-09-03 20:14) ve `crates/actos-types` kaynağına
> karşı doğrulandı.

---

## 1. Backend Faz 18.A'dan doğan eksikler

Canlı `actos-backend/docs/openapi.json` bu uçları/parametreleri doğruluyor
(`python3 -c "import json; ..."` ile ayrıca teyit edildi); `actos-types`
crate'i tipleri zaten taşıyor (path bağımlılığı sayesinde derleniyor) ama
SDK yüzeyi bunları **sunmuyor**.

### 1.1 `client.inbox()` hiç yok

- `src/resources/mod.rs:5-17` — `pub mod` listesinde `inbox` yok.
- `src/client.rs:215-291` — `Actos` üzerinde `auth()`…`meta()` var,
  `inbox()` accessor'ı yok.
- Backend tarafında `InboxResponse`, `NotificationSummary`,
  `MarkAllReadResponse` tipleri hazır:
  `actos-backend/crates/actos-types/src/notification.rs:16-69`.
- Canlı spec'te uçlar var: `GET /me/inbox`, `POST /me/inbox/read`,
  `PATCH /me/inbox/{id}/read` (doğrulama: `openapi.json` paths, bu görev
  sırasında `python3` ile listelendi).
- **Not:** PLAN.md §3 satır 242-245'te tarif edilen yüzey
  (`inbox().list/read/read_all/unread_count`) ile gerçek uçlar arasında
  fark var: PLAN'daki `read_all()…up_to_cursor(..)` `POST /me/inbox/read`
  ile eşleşiyor, ama `read(notification_id)` PLAN'da `PATCH` değil PUT/POST
  gibi anlatılmış; gerçek uç `PATCH /me/inbox/{id}/read`. Uygulayacak ajan
  builder'ı **gerçek HTTP metoduna** (`PATCH`) göre yazmalı, PLAN metnine
  değil.
- `inbox().watch()` (PLAN.md:472-474, yoklama tabanlı stream) henüz hiç
  yok — bu da bu maddenin parçası.
- PLAN.md Faz 12.B (satır 468-479) tüm bu maddeleri `[ ]` bırakmış; bu artık
  doğru **çünkü** kod hâlâ eksik, ama gerekçe metni ("BLOKE — backend Faz
  18.A", satır 463-466) artık geçersiz — blok kalktı, iş sadece kodlanmayı
  bekliyor.

### 1.2 `actors().update_me()` — `.avatar(..)` yok

- `src/resources/actors.rs:59-70` — `update_me()` rustdoc'u satır 63'te
  bilerek "not supported at this time" diyor; `UpdateMeBuilder` struct'ı
  (`src/resources/actors.rs:268-271`) yalnızca `display_name`/`bio` alanları
  taşıyor, `avatar` yok.
- Backend `UpdateProfileRequest.avatar` alanı zaten üç durumlu
  (`Option<Option<String>>`, `double_option` deseniyle):
  `actos-backend/crates/actos-types/src/actor.rs:49` (struct başlangıcı),
  `:67` (`pub avatar: Option<Option<String>>`), `:70` (`fn double_option`).
- **Kritik uygulama notu (kodda doğrulandı):** mevcut `display_name`/`bio`
  implementasyonu (`src/resources/actors.rs:288-296`, `send()` metodu)
  yalnızca `Option<String>` kullanıyor ve yalnızca `Some` durumunda alanı
  gövdeye ekliyor — yani bugün bile **kullanıcı `display_name`/`bio`'yu
  `null` göndererek temizleyemiyor**, sadece "dokunma" ya da "yeni değer
  ata" var. `avatar` alanı **üç durumlu olmak zorunda** (dokunma / null ile
  kaldır / id ile ata — bkz. görev bağlamı). `display_name`/`bio`'nun
  bugünkü iki-durumlu `Option<String>` deseni buraya **kopyalanamaz**;
  `avatar` için ayrı bir enum (ör. `AvatarUpdate::Keep/Clear/Set(String)`)
  ya da `Option<Option<String>>` taşıyan özel bir builder alanı gerekiyor.
  Bu fırsatla `display_name`/`bio`'nun da temizlenebilir hâle getirilip
  getirilmeyeceği ayrı bir karar — PLAN.md bunu zorunlu kılmıyor, ama
  tutarlılık için değerlendirilmeli.
- `src/blocking/mod.rs:698-786` (`BlockingUpdateMeBuilder`) da mevcut
  async builder'a ince bir sarmalayıcı olarak delege ediyor
  (`inner: UpdateMeBuilder<'a>`) — async tarafta `.avatar(..)` eklenince
  blocking tarafta da **elle** karşılık gelen delege metod eklenmeli,
  otomatik gelmiyor (blocking `Deref` değil, metod metod elle yazılmış).

### 1.3 `feed().list()` / `feed().following()` — `.actor_type(..)` yok

- `src/resources/feed.rs:22-28` — `list()` rustdoc'u satır 27'de aynı
  şekilde "intentionally omitted" diyor. `FeedBuilder` struct'ı
  (`src/resources/feed.rs:67-73`) `actor_type` alanı taşımıyor.
  `following()` (aynı dosyada `FollowingFeedBuilder`) için de aynı durum.
- Canlı spec doğrulandı: `GET /feed` ve `GET /feed/following` ikisi de
  `actor_type` parametresi kabul ediyor (`openapi.json`, bu görev
  sırasında `python3` ile listelendi: `['sort', 'window', 'actor_type',
  'cursor', 'limit', 'fields']`).
- Not: `actors().list()` üzerindeki `actor_type` filtresi **zaten var**
  (`src/resources/actors.rs:32-40`, `ListActorsBuilder.actor_type`) — bu
  farklı bir uç (`/actors`), 18.A öncesinden beri mevcut, karıştırılmamalı.
  Eksik olan yalnızca `/feed` ve `/feed/following`.

### 1.4 `/posts/{id}/comments` — `body_html` boolean bayrağı yok

- `src/resources/comments.rs:186-225` — `ListCommentsBuilder`'da `sort`,
  `depth`, `parent`, `limit`, `cursor` var; `body_html` metodu yok.
  `send()` (satır 228-250) ve `stream()` (satır 252-291) da bu parametreyi
  hiç query'e eklemiyor.
- Backend'de bu **bilinçli olarak `?fields=` almıyor** (ağaç yapısını
  bozmamak için), ayrı bir `?body_html=true` bayrağı eklendi — canlı spec
  doğrulandı: `GET /posts/{id}/comments` parametreleri arasında `body_html`
  var (`['id', 'sort', 'depth', 'parent', 'cursor', 'limit', 'body_html']`).
- Bu, diğer üç maddeden farklı olarak PLAN.md'de **hiç anılmıyor** —
  backend PLAN.md'ye göre (`actos-backend/PLAN.md:608-614`) bu ihtiyaç
  "uygulama sonrası bulunan açık boşluk" olarak 18.A'nın ortasında ortaya
  çıktı, dolayısıyla rust `PLAN.md` §3'te (satır 204-209) hiç yer almıyor.
  **`PLAN.md`'ye yeni bir madde olarak eklenmesi gerekiyor**, sadece kod
  değil.

### 1.5 `body_html`/`avatar_url`/`trust_level` — tipler zaten geçiyor, ekstra kod GEREKMİYOR

Doğrulandı, karıştırılmasın:

- `actos_types::content::ContentSummary.body_html`,
  `actos_types::auth::ActorSummary.avatar_url` ve `.trust_level` zaten
  `actos-types` path bağımlılığı üzerinden SDK'da mevcut ve derleniyor —
  `src/resources/posts.rs:290-291,301`'deki `synthesize_partial_post`
  fonksiyonu bu alanlara zaten varsayılan değer atıyor (aksi hâlde sparse
  `?fields=` yanıtlarından `Post` inşa edilemezdi).
- `.fields([...])` builder'ları (`posts().get()`, `feed().list/following()`,
  `tags().posts()`, `saves().list()`, `search().query()`) **rastgele
  string'leri** query'e geçiriyor (bkz. `src/resources/search.rs:63-71`,
  `src/resources/saves.rs:69-77`, `src/resources/tags.rs:150-158`) — yani
  kullanıcı bugün bile `.fields(["body_html"])` yazabilir ve çalışır.
  Bunun için **ek kod gerekmiyor**.
- Tek gerçek eksik `body_html` tarafında **1.4**'teki ayrı boolean
  parametre — çünkü o uç `?fields=` almıyor.

---

## 2. `actos-types` bağımlılığı — yayınlanabilirlik engeli

**Doğrulandı, somut ve tekrarlanabilir:**

- `Cargo.toml:13`:
  ```
  actos-types = { path = "../actos-backend/crates/actos-types", default-features = false }
  ```
  Bu bir **yerel path bağımlılığı**, PLAN.md §0'ın öngördüğü "git bağımlılığı"
  (`{ git = "https://github.com/actos-dev/backend" }`, PLAN.md:23) **değil**.
- Komut çalıştırılıp doğrulandı:
  ```
  $ cargo package --allow-dirty --no-verify
  error: failed to verify manifest at `.../rust/Cargo.toml`
  Caused by:
    all dependencies must have a version requirement specified when packaging.
    dependency `actos-types` does not specify a version
  ```
  Yani `cargo package`/`cargo publish` bugün **fiilen ve kesin olarak
  başarısız oluyor** — bu teorik bir risk değil, komutu çalıştırıp alınan
  gerçek hata.
- **PLAN.md Faz 16'daki iki madde bu yüzden yanlış işaretli** (bkz. §3):
  - Satır 513-514: "Temiz bir projeye `cargo add --git
    https://github.com/actos-dev/rust` ile eklenip örnek çalıştırılır" —
    `[x]`. Bu **yapısal olarak imkânsız**: `path = "../actos-backend/..."`
    yalnızca `rust` deposunun yanına `actos-backend`'in de klonlanmış
    olduğu bir dizin düzeninde çözülür. Temiz bir projede (yalnızca `rust`
    deposu klonlanmış) `cargo add --git` bu path'i bulamaz, derleme
    `error: failed to load source for a dependency` ile patlar. Bu madde
    test edilmiş olamaz ya da test ortamı yanlışlıkla `actos-backend`
    içeriyordu.
  - Satır 515: "`cargo deny check` (lisans + güvenlik) yeşil" — bu kısım
    **doğru**, `cargo deny check` gerçekten 0 çıkış koduyla dönüyor (yalnızca
    `getrandom`/`syn`/`windows-sys` için "duplicate" **uyarısı** var,
    `deny.toml`'da `[bans]` bölümü tanımlanmadığı için bunlar engelleyici
    değil — bilgi amaçlı not, aksiyon gerekmez).
- CI (`\.github/workflows/ci.yml:1-38`) bu path bağımlılığını
  `actos-backend`'i ayrı bir adımda yan dizine checkout ederek "çözüyor"
  (satır 15-19); bu CI'da işe yarar ama **crates.io'da ya da SDK'yı tüketen
  üçüncü bir projede işe yaramaz** — CI'ın yeşil olması paketlenebilirliği
  kanıtlamıyor.
- **Sonuç / önerilen düzeltme (uygulama kararı, bu görevin kapsamı dışı ama
  not düşülüyor):** ya (a) `actos-types`'ı gerçekten `{ git =
  "https://github.com/actos-dev/backend" }` bağımlılığına çevirmek (PLAN'ın
  §0 kararı buydu) ya da (b) PLAN.md'yi güncelleyip "v1'de yalnızca
  monorepo-yanı-monorepo geliştirme desteklenir, git bağımlılığına geçiş
  ayrı bir faz" demek. Şu an kod ile PLAN §0.1 (satır 38-53, "neden git
  bağımlılığı") arasında **doğrudan çelişki** var: gerekçe metni git
  bağımlılığını anlatıyor, kod path kullanıyor.

---

## 3. Planda `[x]` ama kodda eksik/eskimiş

### 3.1 CI'da `cargo deny` adımı yok

- PLAN.md Faz 0, satır 355: "`.github/workflows/ci.yml`: fmt + clippy
  `-D warnings` + test + `cargo deny`. **Yayın job'u yok**" — `[x]`.
- Gerçek dosya `.github/workflows/ci.yml:1-38` yalnızca üç adım içeriyor:
  `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
  **`cargo deny check` adımı hiç yok.** Paketleme engeli (§2) göz önüne
  alınca bu özellikle önemli — CI bugün `cargo deny`'i hiç çalıştırmıyor,
  yerel olarak çalıştırılıp yeşil olduğu bu görev sırasında ayrıca
  doğrulandı ama bu CI'ın garantisi değil, elle yapılan bir kontrol.

### 3.2 Yorum silme placeholder metni eski: `"[silindi]"` yerine `"[deleted]"` olmalı

- Backend PLAN.md'nin "Hata metinleri İngilizceye" maddesi (18.A kapsamında,
  `actos-backend/PLAN.md:818-856`, hepsi `[x]`) gövde-içi Türkçe yer
  tutucuları da kapsıyordu ve backend kodu bunu uyguladı — doğrulandı:
  `actos-backend/crates/actos-api/src/routes/posts.rs:126,300` artık
  `"[deleted]"` yazıyor, `"[silindi]"` **hiçbir yerde kalmadı**
  (`grep -rn '"\[silindi\]"'` backend'de sıfır sonuç verdi).
- Rust SDK bunu **yakalamamış**, hâlâ eski Türkçe metni referans alıyor:
  - `src/resources/comments.rs:76` — rustdoc: `` `comment.body =
    "[silindi]"` `` (yanlış, olması gereken `"[deleted]"`).
  - `tests/comments_tests.rs:231,235,247` — mock test hem yorumda hem
    assertion'da `"[silindi]"` kullanıyor. Bu bir wiremock testi olduğu
    için CI'da **kırılmıyor** (kendi kendine tutarlı mock), ama gerçek
    backend'e karşı çalışan `tests/contract.rs` (Faz 13, uçtan uca
    senaryo) bu davranışı test etmiyor gibi görünüyor — `contract.rs`'te
    silinmiş yorum/post placeholder metni için bir assertion bulunamadı,
    yani bu tutarsızlık bugün **hiçbir testte** yakalanmıyor.
  - Düzeltme küçük (rustdoc + mock string'i "[deleted]" yapmak) ama gerçek
    bir hata: SDK dokümantasyonu kullanıcıya yanlış bir sözleşme
    (Türkçe placeholder) vaat ediyor.
- Aynı grep `src/`, `tests/`, `examples/`, `README.md`, `CHANGELOG.md`
  genelinde çalıştırıldı, başka "silindi" referansı çıkmadı (yalnızca bu
  dört satır).

### 3.3 NOTES.md §2 artık eski

- `NOTES.md:16-30` hâlâ `inbox.*`/`verifications.*`, `.avatar(..)`,
  `.actor_type(..)`'ın "backend Faz 18.A tamamlanana kadar" ertelendiğini
  anlatıyor. Faz 18.A bitti (backend `PLAN.md:856` — `[x] Commit (18.A)`),
  bu üçünden ikisi (`avatar`, `actor_type`) artık kodlanabilir durumda,
  yalnızca `verifications.*` hâlâ geçerli sebeple ertelenmiş durumda (bkz.
  §4). `NOTES.md` bunu ayırt etmiyor — güncellenmesi gerekiyor ama bu görev
  kapsamında **değiştirilmedi** (görev talimatı: yalnızca
  `YAPILACAKLAR.md` yazılacak).

---

## 4. Plandan düşülmesi gerekenler (yapılmayacak)

### 4.1 `verifications().create/check/list/delete` — YAPILMAYACAK, plandan düşülmeli

- PLAN.md'de şu an işaretsiz iki yerde geçiyor:
  - Satır 75: özet tablo, "`inbox.*` ve `verifications.*` | Faz 12.B" —
    ikisini aynı kefeye koyuyor, **yanlış**: `inbox.*` gerçekten bekliyordu
    ve artık yapılabilir; `verifications.*` hiç bekleme durumunda değildi,
    backend'de **süresiz ertelendi**.
  - Satır 247-249: API yüzeyi haritasında somut imzalarla duruyor
    (`create(domain, method)`, `check(id)`, `list()/delete(id)`).
  - Satır 475: Faz 12.B checklist maddesi, `[ ]
    verifications().create/check/list/delete (alan adı doğrulaması)`.
- **Doğrulanmış gerçek durum:** backend `NOTES.md` §9.2
  (`actos-backend/NOTES.md:288-379`) "Alan adı doğrulaması — ERTELENDİ
  (2026-09-03)" başlığı altında, **kullanıcı kararıyla v1 kapsamı dışına**
  alındığını, gerekçesinin SSRF yüzeyi ve DNS-rebinding TOCTOU riski
  olduğunu net biçimde kayda geçiriyor (§9.2.4, §9.2.5). Backend
  `PLAN.md:708-718` bunu "Kullanıcı kararı" olarak özetliyor; aynı dosyada
  (satır 798-817) tam tasarım hâlâ `[ ]` olarak saklı duruyor ama bu **bir
  yapılacaklar listesi değil, gün geldiğinde sıfırdan düşünülmesin diye
  saklanan bir taslak**.
  - Sözü edilen uçlar (`POST /me/verifications`, `POST
    /me/verifications/{id}/check`, `GET`/`DELETE /me/verifications`) canlı
    `openapi.json`'da **yok** (bu görev sırasında `python3` ile arandı,
    `/me/inbox*` dışında `verif` geçen hiçbir path bulunamadı).
  - `actos-types` crate'inde de bu uçlara karşılık gelen bir istek/yanıt
    tipi yok (`grep -rn verif` backend `crates/actos-types/src` içinde
    boş sonuç).
- **Aksiyon:** PLAN.md'nin yönetici tarafından güncellenmesi gerekiyor —
  satır 75'teki tablo satırından `verifications.*` çıkarılmalı (ya da
  ayrı, "ertelendi" diye işaretli bir satıra taşınmalı), satır 247-249
  API haritasından silinmeli, satır 475'teki checklist maddesi ya
  tamamen kaldırılmalı ya da açıkça "ERTELENDİ — backend NOTES.md §9.2,
  v1 kapsamı dışı" notuyla değiştirilmeli. **Bu görev kapsamında PLAN.md'ye
  dokunulmadı** (talimat gereği), yalnızca burada işaretleniyor.

---

## 5. Diğer teknik borç

### 5.1 SDK'nın kendi iç hata mesajları Türkçe

- `src/error.rs:113,117,121,125` — `Error::Transport`, `Error::Decode`,
  `Error::Config`, `Error::Io` varyantlarının `#[error(...)]` mesajları
  sırasıyla `"taşıma hatası: {0}"`, `"yanıt çözümlenemedi: {0}"`,
  `"yapılandırma hatası: {0}"`, `"dosya okuma hatası: {0}"`.
- Bu, backend'in `detail` metinlerinden **bağımsız** bir konu — backend
  zaten İngilizceye geçti ve SDK onu olduğu gibi taşıyor (`detail:
  Option<String>` üzerinden, çeviri yapmıyor, bu doğru davranış). Ama
  SDK'nın **kendi ürettiği** dört hata varyantının mesaj metni hâlâ Türkçe,
  oysa crate'in geri kalanı (README, rustdoc, `lib.rs`, `CHANGELOG.md`,
  tüm public API isimleri) tamamen İngilizce. Kullanıcı `err.to_string()`
  çağırdığında karışık dilli bir çıktı alıyor.
  Test edilen tam metinler: `tests/../src/error.rs:548-555` (birim
  testlerinde bu Türkçe string'ler doğrudan assert ediliyor, yani
  değiştirilirse testler de güncellenmeli).
- PLAN.md'de bunu zorunlu kılan açık bir madde yok, ama görev bağlamındaki
  "Tüm hata metinleri… artık İngilizce" ilkesiyle ve crate'in genel
  İngilizce duruşuyla çelişiyor. Düşük risk, kolay düzeltme, önerilir.

### 5.2 `display_name`/`bio` alanları temizlenemiyor (üç durumlu değil)

- Bkz. §1.2 içindeki not: `src/resources/actors.rs:268-303`. Backend
  `UpdateProfileRequest.display_name`/`.bio` zaten `Option<Option<String>>`
  (`double_option`) ama SDK builder'ı yalnızca `Option<String>` taşıdığı
  için kullanıcı bu alanları asla `null` ile temizleyemiyor. PLAN.md bunu
  açıkça istemiyor, bu yüzden "eksik" değil, ama `avatar` eklenirken aynı
  triple-state ihtiyacı ortaya çıkacağı için bu tutarsızlık görünür
  hâle gelecek — aynı geçişte ele alınması mantıklı.

### 5.3 `cargo deny` "duplicate" uyarıları (bilgi amaçlı, engelleyici değil)

- `getrandom` (0.2.17 / 0.4.3), `syn` (2.0.119 / 3.0.4), `windows-sys`
  (0.52.0 / 0.61.2) için çoklu sürüm uyarısı var (`cargo deny check`
  çıktısı, bu görev sırasında çalıştırıldı). `deny.toml`'da `[bans]`
  bölümü olmadığı için varsayılan davranışla bunlar yalnızca **uyarı**,
  `check` komutu 0 ile dönüyor. Aksiyon gerekmez, bilgi notu.

---

## 6. Sıra önerisi

1. **`actos-types` bağımlılığını çöz (§2).** Diğer her şeyden önce —
   paketleme bugün kesin olarak kırık ve bu, kod eklemekten önce mimari
   bir karar gerektiriyor (git bağımlılığına geçiş mi, yoksa PLAN.md'nin
   "v1 yalnızca monorepo" diye güncellenmesi mi). Bu karar netleşmeden
   Faz 16 checklist'i tekrar "yeşil" sayılamaz.
2. **`feed().actor_type(..)` ve `actors().update_me().avatar(..)` (§1.2,
   §1.3).** İkisi de küçük, iyi izole builder değişiklikleri; spec ve
   backend tipleri hazır. `avatar` için §1.2'deki triple-state uyarısına
   dikkat.
3. **`client.inbox()` (§1.1).** En büyük yeni yüzey; `NotificationSummary`
   / `InboxResponse` / `MarkAllReadResponse` zaten `actos-types`'ta hazır.
   PLAN.md'deki `read()` HTTP metod varsayımını gerçek spec'e göre düzeltmeyi
   unutma (`PATCH`, PUT/POST değil).
4. **`comments().list().body_html(bool)` (§1.4).** Küçük ama PLAN.md'ye
   önce yeni bir madde olarak eklenmesi gerekiyor (bugün orada yok).
5. **CI'a `cargo deny` adımını ekle (§3.1)** — ucuz, hemen yapılabilir,
   paketleme kararından bağımsız.
6. **`"[silindi]"` → `"[deleted]"` (§3.2)** — küçük metin düzeltmesi,
   rustdoc + bir test dosyası.
7. **PLAN.md'nin yönetici tarafından güncellenmesi** — `verifications.*`
   maddesinin düşülmesi (§4.1) ve `NOTES.md`'nin tazelenmesi (§3.3); bu
   ikisi kod değil dokümantasyon işi ama unutulursa bir sonraki tur yine
   aynı yanlış varsayımla başlar.
8. **§5'teki teknik borç** (Türkçe iç hata mesajları, `display_name`/`bio`
   temizleme) — istenirse `avatar` işiyle aynı geçişte, değilse ayrı, düşük
   öncelik.

---

## Doğrulanamayanlar / not düşülenler

- PLAN.md Faz 16 satır 513-514'ün "temiz projede test edildi" iddiası
  fiilen yanlış olduğu §2'de gösterildi, ama bu maddenin **nasıl**
  `[x]` işaretlendiği (hangi ortamda, hangi komutla) doğrulanamadı —
  muhtemelen `actos-backend` yan dizinde varken test edilip yanlış
  genellenmiş.
- README.md'deki "94 Unit & Integration Tests" rakamı bu görev sırasında
  çalıştırılan `cargo test --all-features` çıktısıyla tam eşleşmedi (ufak
  fark, saymada belirsizlik — doctest'lerin dahil edilip edilmediği net
  değil). Küçük ve önemsiz bir sapma olduğu için ayrı bir madde
  açılmadı, burada yalnızca not düşülüyor.
