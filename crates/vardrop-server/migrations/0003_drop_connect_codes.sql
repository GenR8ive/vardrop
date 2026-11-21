-- Bağlantı kodu akışı kaldırıldı: istemci (exe) bağlantısını PAT ile kurar
-- (panel → Tokenlar → read PAT → exe kurulum ekranına sunucu adresi + token).
-- İmzalı kod / tarayıcı onay uçları ve enroll kayıtları artık kullanılmıyor.

DROP TABLE IF EXISTS connect_codes;
