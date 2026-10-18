#include <windows.h>
#include <shlwapi.h>
#include <strsafe.h>
#include <string>


extern HMODULE g_hModule;

class PathHelper {
public:
    /**
     * @brief Constructs the full path to wezterm-gui.exe.
     *
     * This function retrieves the module's directory path, appends "\\wezterm-gui.exe"
     * to it, and returns the full path as a std::wstring.
     *
     * @return std::wstring The full path to wezterm-gui.exe.
     */
    std::wstring ModulePath() {
        WCHAR modulePath[MAX_PATH];
        if (GetModuleFileNameW(g_hModule, modulePath, MAX_PATH)) {
            PathRemoveFileSpecW(modulePath);
            StringCchCatW(modulePath, ARRAYSIZE(modulePath), L"\\wezterm-gui.exe");
            return std::wstring(modulePath);
        }
        return L"";
    }

    /**
     * @brief Formats the icon resource path using the module path.
     *
     * This function takes the module path and formats it to use the embedded resource,
     * storing the result in iconResourcePath.
     *
     * @param modulePath The module path.
     * @param iconResourcePath The buffer to store the formatted icon resource path.
     * @param size The size of the iconResourcePath buffer.
     * @return HRESULT The result of the formatting operation.
     */
    HRESULT FormatIconResourcePath(const WCHAR* modulePath, WCHAR* iconResourcePath, size_t size) {
        StringCchPrintfW(iconResourcePath, size, L"%s,0", modulePath);
        return S_OK;
    }
};
