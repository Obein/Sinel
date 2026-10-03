let currentLang = 'zh';

function setLanguage(lang) {
  currentLang = lang;
  document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';

  document.querySelectorAll('[data-en]').forEach(el => {
    el.textContent = el.getAttribute('data-' + lang);
  });

  const btnEn = document.getElementById('btn-en');
  const btnZh = document.getElementById('btn-zh');
  if (btnEn) btnEn.classList.toggle('active', lang === 'en');
  if (btnZh) btnZh.classList.toggle('active', lang === 'zh');

  try {
    localStorage.setItem('sinel_lang', lang);
  } catch (e) {}
}

function copyProxyUrl() {
  const input = document.getElementById('shareUrl');
  if (!input) return;
  input.select();
  navigator.clipboard.writeText(input.value).then(() => {
    const textSpan = document.getElementById('copyBtnText');
    const copyBtn = document.getElementById('copyBtn');
    if (!textSpan || !copyBtn) return;

    const originalText = textSpan.textContent;
    textSpan.textContent = currentLang === 'zh' ? '已复制' : 'Copied';
    copyBtn.classList.add('copied');

    setTimeout(() => {
      textSpan.textContent = originalText;
      copyBtn.classList.remove('copied');
    }, 1500);
  });
}

(function initLang() {
  try {
    const saved = localStorage.getItem('sinel_lang');
    if (saved && (saved === 'en' || saved === 'zh')) {
      setLanguage(saved);
      return;
    }
  } catch (e) {}

  const userLang = (navigator.language || navigator.userLanguage || '').toLowerCase();
  if (!userLang.startsWith('zh')) {
    setLanguage('en');
  }
})();
