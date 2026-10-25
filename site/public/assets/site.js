// Two small helpers, no dependencies: copy buttons and scroll reveal.

for (const button of document.querySelectorAll('[data-copy]')) {
  const label = button.textContent;
  button.addEventListener('click', async () => {
    const source = document.getElementById(button.dataset.copy);
    if (!source) return;
    try {
      await navigator.clipboard.writeText(source.innerText.replace(/\n+$/, ''));
      button.textContent = button.dataset.done || 'Copied';
    } catch {
      button.textContent = button.dataset.fail || 'Select and copy';
    }
    setTimeout(() => {
      button.textContent = label;
    }, 1800);
  });
}

const items = document.querySelectorAll('[data-reveal]');
if ('IntersectionObserver' in window && items.length) {
  document.documentElement.classList.add('reveal-ready');
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-revealed');
          observer.unobserve(entry.target);
        }
      }
    },
    { rootMargin: '0px 0px -8% 0px' },
  );
  items.forEach((el) => observer.observe(el));
}
