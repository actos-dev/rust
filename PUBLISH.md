# Yayın (publish) — Rust SDK

> Durum: **yayınlanmadı, iki engel var.** Kod tarafı hazır (`cargo test
> --all-features` yeşil, `clippy -D warnings` ve `cargo deny check` temiz).
> Kalan iş paketleme.
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

`rust/Cargo.toml` ve `cli/Cargo.toml` **ikisi de** `name = "actos"` diyor.
crates.io'da bu adın tek sahibi olur.

Alışıldık çözüm: kütüphane `actos` adını alır, CLI `actos-cli` olarak
yayınlanır ama `[[bin]] name = "actos"` sayesinde yine `actos` komutu
olarak kurulur. Karar verilmedi.

## Yayın iş akışı henüz yazılmadı

`.github/workflows/` altında yalnızca `ci.yml` var. Yukarıdaki üç madde
çözülünce tag tetikleyicili bir publish adımı eklenecek.

## Sıradaki adım

1. `actos-types`'ı Apache-2.0'a çevir (backend repo'su).
2. `actos-types`'ı crates.io'ya yayınla.
3. Buradaki path bağımlılığını sürüm bağımlılığına çevir.
4. Ad çakışmasını çöz (`actos` kime gidiyor).
5. Publish workflow'u + `v0.1.0` tag'i.
