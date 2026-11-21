-- Secret erişim PIN'i (opsiyonel): belirleyen kullanıcı için secret okuma
-- yalnız PIN ile açılan 10 dakikalık unlock penceresinde serbesttir.
-- NULL = PIN yok (secret okuma davranışı değişmez).

ALTER TABLE users ADD COLUMN pin_hash TEXT;
