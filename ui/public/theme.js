// Run before the app loads so a saved light theme never flashes dark content.
(() => {
  let theme = 'dark';
  try {
    if (localStorage.getItem('schematlas.theme') === 'light') theme = 'light';
  } catch {
    // Storage can be unavailable; the switch still works for this session.
  }
  document.documentElement.dataset.theme = theme;
  document.querySelector('meta[name="color-scheme"]').content = theme;
  document.querySelector('meta[name="theme-color"]').content =
    theme === 'light' ? '#f7f8fa' : '#070809';
})();
