import { ShortcutInfo } from '../hooks/useKeyboardShortcuts'

interface KeyboardShortcutsHelpProps {
  shortcuts: ShortcutInfo[]
  onClose: () => void
}

export default function KeyboardShortcutsHelp({ shortcuts, onClose }: KeyboardShortcutsHelpProps) {
  // Group shortcuts by category
  const actionShortcuts = shortcuts.filter(s =>
    ['fold', 'call_check', 'bet', 'raise', 'all_in'].includes(s.action)
  )
  const betAmountShortcuts = shortcuts.filter(s =>
    ['min', 'half_pot', 'pot', 'max'].includes(s.action)
  )
  const adjustShortcuts = shortcuts.filter(s =>
    ['increase', 'decrease'].includes(s.action)
  )

  return (
    <div className="bg-gray-800/95 rounded-lg p-3 mt-2 border border-gray-600 text-sm">
      <div className="flex justify-between items-center mb-2">
        <span className="font-bold text-white">Keyboard Shortcuts</span>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-white text-lg leading-none"
        >
          ×
        </button>
      </div>

      <div className="grid grid-cols-3 gap-3">
        {/* Actions */}
        <div>
          <div className="text-gray-400 text-xs mb-1 font-semibold">Actions</div>
          {actionShortcuts.map(shortcut => (
            <div key={shortcut.action} className="flex items-center gap-2 py-0.5">
              <kbd className="bg-gray-700 px-1.5 py-0.5 rounded text-xs font-mono text-yellow-400 min-w-[20px] text-center">
                {shortcut.key}
              </kbd>
              <span className="text-gray-300 text-xs">{shortcut.description}</span>
            </div>
          ))}
        </div>

        {/* Bet Amounts */}
        <div>
          <div className="text-gray-400 text-xs mb-1 font-semibold">Bet Amount</div>
          {betAmountShortcuts.map(shortcut => (
            <div key={shortcut.action} className="flex items-center gap-2 py-0.5">
              <kbd className="bg-gray-700 px-1.5 py-0.5 rounded text-xs font-mono text-yellow-400 min-w-[20px] text-center">
                {shortcut.key}
              </kbd>
              <span className="text-gray-300 text-xs">{shortcut.description}</span>
            </div>
          ))}
        </div>

        {/* Adjust */}
        <div>
          <div className="text-gray-400 text-xs mb-1 font-semibold">Adjust</div>
          {adjustShortcuts.map(shortcut => (
            <div key={shortcut.action} className="flex items-center gap-2 py-0.5">
              <kbd className="bg-gray-700 px-1.5 py-0.5 rounded text-xs font-mono text-yellow-400 min-w-[20px] text-center">
                {shortcut.key}
              </kbd>
              <span className="text-gray-300 text-xs">{shortcut.description}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="mt-2 pt-2 border-t border-gray-600 text-xs text-gray-400 text-center">
        Shortcuts are active only when it's your turn
      </div>
    </div>
  )
}
