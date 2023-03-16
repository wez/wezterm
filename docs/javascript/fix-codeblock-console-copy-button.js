document.addEventListener("DOMContentLoaded", function() {
  fixCopyOnlyUserSelectable();
});

function fixCopyOnlyUserSelectable() {
  buttonsToFix = document.querySelectorAll(
    '.language-console button.md-clipboard');
  if (buttonsToFix.length)
    console.log('Fixing copy-to-clipboard text of console code-blocks.');
  buttonsToFix.forEach((btn) => {
    var content = extractUserSelectable(btn.dataset.clipboardTarget);
    btn.dataset.clipboardText = content;
  });
}

function extractUserSelectable(selector) {
  var result = '';
  var element = document.querySelector(selector);

  // Attempt to remove the non-selectable sections based on style,
  // but we haven't seen this work reliably...
  element.childNodes.forEach((child) => {
    if (child instanceof Element) {
      var s=window.getComputedStyle(child);
      if (s.getPropertyValue('user-select') == 'none' ||
        s.getPropertyValue('-webkit-user-select') == 'none' ||
        s.getPropertyValue('-ms-user-select') == 'none')
      {
        return;
      }
    }
    result += child.textContent;
  });

  // ... so we fall back to simple but effective:
  // remove "$ " and "# " prompt at start of lines in code
  result = result.replace(/^[\s]?[\$#]\s+/gm, "")

  // remove empty lines
  result = result.replace(/^\s*\n/gm, '')
  return result;
}
