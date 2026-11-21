; Vardrop NSIS kurulum kancası (tauri.conf.json → bundle.windows.nsis.installerHooks).
; PAT akışı: kurulum dosyası ham iner; bağlantı kodu/kod taşıma yoktur.
; İstemciyi kurduktan sonra kullanıcı exe'nin kurulum ekranına sunucu adresini
; ve panelden ürettiği read PAT'i girer. Kancanın yapacak işi kalmadı;
; yerinde tutulur (NSIS hook sözdizimi boş makro kabul etmez).
;
; NSIS buluşsal değişkenleri: $R0/$R1/$R2 kanca içinde geçici olarak kullanılır.
; installMode = currentUser olduğundan yükseltme/yeniden başlatma yoktur.

!macro NSIS_HOOK_POSTINSTALL
  ; PAT akışında kurulum kancası iş yapmaz.
!macroend
