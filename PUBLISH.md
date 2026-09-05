# Yayın (publish) — Rust SDK

> Durum: **yayınlandı ✅ v0.1.0 crates.io'da** (2026-09-05).
> `cargo add actos` çalışıyor; `Actos::builder().build()` ile doğrulandı.
>
> Son güncelleme: 2026-09-05.

## Hazır olanlar

- `CARGO_REGISTRY_TOKEN` GitHub secret'ı `actos-dev/rust` reposuna kondu.
- crates.io'da `actos`, `actos-types`, `actos-sdk`, `actos-cli` adlarının
  **hepsi müsait** (kontrol edildi 2026-09-05).

## ENGEL 1 — `actos-types` path bağımlılığı

`Cargo.toml:13`:

```toml
actos-types = { path = "../actos-backend/crates/actos-types", default-features = false }
```

`cargo publish` path bağımlılığı olan bir crate'i **kabul etmez**. Bu yüzden
`cargo package` ve `cargo install` bugün fiilen kırık.

Çözüm: `actos-types` backend repo'sunun bir parçası ve backend artık public
(`github.com/actos-dev/backend`, `v0.1.0`). O crate crates.io'ya
yayınlanır, sonra buradaki bağımlılık sürüm bağımlılığına çevrilir:

```toml
actos-types = { version = "0.1", default-features = false }
```

**Sıra önemli:** önce `actos-types`, sonra bu SDK. Aynısı `cli` için de
geçerli — o da aynı path bağımlılığını taşıyor.

## ENGEL 2 — lisans çakışması

```
actos-types (backend workspace'inden miras):  AGPL-3.0-only
bu SDK (Cargo.toml):                          Apache-2.0
```

**Apache-2.0 bir crate, AGPL bir kütüphaneye bağlanıp Apache-2.0
kalamaz.** AGPL copyleft ve bulaşıcı: bu SDK'yı kullanan herkesin
uygulaması da AGPL olmak zorunda kalırdı. Bir SDK için bu genelde istenenin
tam tersi — SDK'lar tam bu yüzden izin verici lisansla dağıtılır.

**Karar verildi (kullanıcı, 2026-09-05): `actos-types` Apache-2.0'a
çevrilecek.** Sunucu AGPL kalır, istemci tarafındaki paylaşılan tipler izin
verici olur — yaygın ve temiz desen.

Uygulaması backend repo'sunda: `crates/actos-types/Cargo.toml` şu an
`license.workspace = true` ile AGPL'i miras alıyor; kendi
`license = "Apache-2.0"` satırını ve kendi LICENSE dosyasını alması
gerekiyor. Backend'in `YAPILACAKLAR.md`'sinde de kayıtlı.

## Karar bekleyen — crates.io ad çakışması

`rust/Cargo.toml` ve `cli/Cargo.toml` **ikisi de** `name = "actos"` diyordu.
crates.io'da bu adın tek sahibi olur.

Alışıldık çözüm: kütüphane `actos` adını alır, CLI `actos-cli` olarak
yayınlanır ama `[[bin]] name = "actos"` sayesinde yine `actos` komutu
olarak kurulur. Karar verilmedi.

> ✅ **ÇÖZÜLDÜ (2026-09-05):** `actos` kütüphaneye gitti (bu repo). CLI
> `actos-cli` olarak yayınlanacak; `cli/Cargo.toml` zaten `[[bin]] name =
> "actos"` içerdiğinden kullanıcı yine `actos` komutunu alacak.

## Yayın iş akışı

`.github/workflows/publish.yml` eklendi — `workflow_dispatch` (elle)
tetiklemeli, `dry_run` input'lu. `cargo publish --dry-run` + koşullu
gerçek yayın; token `CARGO_REGISTRY_TOKEN` repo secret'ından okunur.

> ✅ İlk sürüm (v0.1.0) bu workflow ile yayınlandı. Not: publish işinde
> `rust-cache` kullanılmıyor — tek seferlik tam derlemede cache istemek
> `Post Run` ENOENT (`tests/trybuild`/`tests/target`) log kirletiyordu,
> kaldırıldı.

## Sıradaki adım

1. ~~`actos-types`'ı Apache-2.0'a çevir (backend repo'su).~~ ✅
2. ~~`actos-types`'ı crates.io'ya yayınla.~~ ✅
3. ~~Path bağımlılığını sürüm bağımlılığına çevir.~~ ✅ (`version = "0.1"`)
4. ~~Ad çakışmasını çöz (`actos` kime gidiyor).~~ ✅ (kütüphaneye)
5. ~~Publish workflow'u + `v0.1.0` tag'i.~~ ✅ (workflow ile yayınlandı)

Bu repo için yayın bitti. Fikir (CLI'nın bu SDK'ya geçmesi) `cli/PUBLISH.md`'de
saklı; CLI adımında ele alınacak.
