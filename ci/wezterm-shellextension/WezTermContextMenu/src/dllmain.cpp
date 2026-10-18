#include <wrl/module.h>
#include <wrl/implements.h>
#include <shobjidl_core.h>
#include <wil/resource.h>
#include <Shellapi.h>
#include <Shlwapi.h>
#include <Strsafe.h>
#include <VersionHelpers.h>
#include <Shobjidl.h>
#include <string>

#include "../include/helper.h"

using namespace Microsoft::WRL;
HMODULE g_hModule = nullptr;


/**
 * @brief The pragma directives below are used to instruct the linker to export the specified functions 
 * with the correct names and calling conventions. This ensures that the functions are available for 
 * external use without needing a .def file.
 */
#pragma comment(linker, "/export:DllCanUnloadNow=DllCanUnloadNow")
#pragma comment(linker, "/export:DllGetClassObject=DllGetClassObject")
#pragma comment(linker, "/export:DllGetActivationFactory=DllGetActivationFactory")

/**
 * @brief DllGetActivationFactory retrieves the activation factory for the specified class.
 * 
 * This function is called to get the activation factory for the specified class identifier (CLSID).
 * 
 * @param activatableClassId The class identifier.
 * @param factory Output parameter to receive the activation factory.
 * @return HRESULT indicating success (S_OK) or failure.
 */
extern "C" HRESULT DllGetActivationFactory(HSTRING activatableClassId, IActivationFactory** factory) {
    return Module<ModuleType::InProc>::GetModule().GetActivationFactory(activatableClassId, factory);
}

/**
 * @brief DllCanUnloadNow checks if the DLL can be unloaded from memory.
 * 
 * This function is called to check whether the DLL can be safely unloaded from memory.
 * 
 * @return S_OK if the DLL can be unloaded, S_FALSE otherwise.
 */
extern "C" HRESULT DllCanUnloadNow() {
    return Module<InProc>::GetModule().GetObjectCount() == 0 ? S_OK : S_FALSE;
}

/**
 * @brief DllGetClassObject retrieves the class factory for the specified class.
 * 
 * This function is called to get the class factory for the specified class identifier (CLSID).
 * 
 * @param rclsid The class identifier.
 * @param riid The interface identifier.
 * @param instance Output parameter to receive the class factory instance.
 * @return HRESULT indicating success (S_OK) or failure.
 */
extern "C" HRESULT DllGetClassObject(REFCLSID rclsid, REFIID riid, void** instance) {
    return Module<InProc>::GetModule().GetClassObject(rclsid, riid, instance);
}


BOOL APIENTRY DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
    if (ul_reason_for_call == DLL_PROCESS_ATTACH) {
        g_hModule = hModule;
    }
    return TRUE;
}

/**
 * @brief WezTermCommand class implements the IExplorerCommand and IObjectWithSite interfaces.
 */
class WezTermCommand : public RuntimeClass<RuntimeClassFlags<ClassicCom>, IExplorerCommand, IObjectWithSite> {
public:
	// IExplorerCommand methods
/**
* @brief GetTitle retrieves the title of the context menu item.
*
* @param items The selected items in the shell.
* @param name Output parameter to receive the title.
* @return HRESULT indicating success or failure.
*/
	IFACEMETHODIMP GetTitle(_In_opt_ IShellItemArray* items, _Outptr_result_nullonfailure_ PWSTR* name) {
		*name = nullptr;
		auto title = wil::make_cotaskmem_string_nothrow(L"Open WezTerm here");
		RETURN_IF_NULL_ALLOC(title);
		*name = title.release();
		return S_OK;
	}
/**
* @brief Retrieves the icon path for the specified shell items.
*
* This method computes the full path to wezterm-gui.exe using the PathHelper class,
* formats the icon resource path, and returns it via the iconPath parameter.
*
* @param items A pointer to an IShellItemArray representing the selected shell items.
* @param iconPath A pointer to a PWSTR that will receive the icon path.
* @return HRESULT The result of the operation.
*/
    IFACEMETHODIMP GetIcon(_In_opt_ IShellItemArray* items, _Outptr_result_nullonfailure_ PWSTR* iconPath) {
        *iconPath = nullptr;

        PathHelper pathHelper;
        std::wstring modulePath = pathHelper.ModulePath();

        if (!modulePath.empty()) {
            // Format icon path to use the embedded resource
            WCHAR iconResourcePath[MAX_PATH];
            pathHelper.FormatIconResourcePath(modulePath.c_str(), iconResourcePath, ARRAYSIZE(iconResourcePath));

            auto iconPathStr = wil::make_cotaskmem_string_nothrow(iconResourcePath);
            if (iconPathStr) {
                *iconPath = iconPathStr.release();
                return S_OK;
            }
        }

        return E_FAIL;
    }

	IFACEMETHODIMP GetToolTip(_In_opt_ IShellItemArray*, _Outptr_result_nullonfailure_ PWSTR* infoTip) { *infoTip = nullptr; return E_NOTIMPL; }
	IFACEMETHODIMP GetCanonicalName(_Out_ GUID* guidCommandName) { *guidCommandName = GUID_NULL; return S_OK; }
/**
* @brief GetState retrieves the state of the context menu item.
*
* This method determines the state of the context menu item based on the provided selection and system conditions.
*
* @param selection The selected items in the shell.
* @param okToBeSlow Indicates whether it's acceptable to be slow.
* @param cmdState Output parameter to receive the state of the context menu item (EXPCMDSTATE).
* @return HRESULT indicating success (S_OK) or failure.
*/
    IFACEMETHODIMP GetState(_In_opt_ IShellItemArray* selection, _In_ BOOL okToBeSlow, _Out_ EXPCMDSTATE* cmdState)
    {
        *cmdState = ECS_ENABLED;
        return S_OK;
    }
/**
* @brief Invoke executes the context menu item action.
*
* This method is called to perform the action associated with the context menu item. It displays debug messages
* for invalid arguments or if there are no items to process. It retrieves the file path of the selected item,
* extracts information about the file.
*
* @param selection The selected items in the shell.
* @param bindContext The bind context.
* @return HRESULT indicating success (S_OK) or failure.
*/

    IFACEMETHODIMP Invoke(_In_opt_ IShellItemArray* selection, _In_opt_ IBindCtx*) noexcept try {
        if (!selection) {
            // Debug message
            MessageBox(nullptr, L"Selection is nil. Please select a valid item.", L"WezTerm Shell Extension", MB_OK);
            return E_INVALIDARG;
        }

        DWORD count;
        RETURN_IF_FAILED(selection->GetCount(&count));

        if (count == 0) {
            // Debug message
            MessageBox(nullptr, L"No items to process", L"WezTerm Shell Extension", MB_OK);
            return S_OK; // No items to process
        }

        if (count > 1) {
            // Inform the user that only the first item will be processed
            MessageBox(nullptr, L"Multiple items selected. Only the first item will be processed.", L"WezTerm Shell Extension", MB_OK);
        }

        // Process the first item
        ComPtr<IShellItem> item;
        RETURN_IF_FAILED(selection->GetItemAt(0, &item));

        // Get the absolute path of the parent directory of the selected item
        PWSTR parentDirPath;
        RETURN_IF_FAILED(item->GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING, &parentDirPath));
        wil::unique_cotaskmem_string parentDirPathCleanup(parentDirPath);

        // Debug: Display the absolute directory path in a message box
        std::wstring message = L"Directory path: " + std::wstring(parentDirPath);


        // Use PathHelper to get the directory containing the DLL as the base path for wezterm-gui.exe
        PathHelper pathHelper;
        std::wstring wezExePath = pathHelper.ModulePath();

        if (wezExePath.empty()) {
            MessageBox(nullptr, L"Failed to construct wezterm-gui.exe path", L"Error", MB_OK | MB_ICONERROR);
            return E_FAIL;
        }

        // Prepare the command-line argument (directory path)
        std::wstring commandLineArgs = L"start --no-auto-connect --cwd \"" + std::wstring(parentDirPath) + L"\"";

        // Launch wezterm-gui.exe with the directory path as a command-line argument
        if (!ShellExecute(nullptr, L"open", wezExePath.c_str(), commandLineArgs.c_str(), nullptr, SW_SHOWNORMAL)) {
            // Retrieve the error code from GetLastError
            DWORD errorCode = GetLastError();

            // Convert error code
            LPVOID lpMsgBuf;
            FormatMessage(
                FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                nullptr,
                errorCode,
                MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
                (LPWSTR)&lpMsgBuf,
                0, nullptr);

            // Prepare the error message
            std::wstring errorMessage = L"Failed to execute wezterm-gui.exe\n";
            errorMessage += L"Executable Path: " + wezExePath + L"\n";
            errorMessage += L"Error: " + std::wstring((wchar_t*)lpMsgBuf);

            // Display the error message with additional context
            MessageBox(nullptr, errorMessage.c_str(), L"Error", MB_OK | MB_ICONERROR);

            // Free the message buffer allocated by FormatMessage
            LocalFree(lpMsgBuf);

            return E_FAIL;
        }
		
        return S_OK;
    }
    CATCH_RETURN();

	IFACEMETHODIMP GetFlags(_Out_ EXPCMDFLAGS* flags) { *flags = ECF_DEFAULT; return S_OK; }
	IFACEMETHODIMP EnumSubCommands(_COM_Outptr_ IEnumExplorerCommand** enumCommands) { *enumCommands = nullptr; return E_NOTIMPL; }

	// IObjectWithSite methods
	IFACEMETHODIMP SetSite(_In_ IUnknown* site) noexcept { m_site = site; return S_OK; }
	IFACEMETHODIMP GetSite(_In_ REFIID riid, _COM_Outptr_ void** site) noexcept { return m_site.CopyTo(riid, site); }

protected:
	ComPtr<IUnknown> m_site;
};

class __declspec(uuid("7A1E471F-0D43-4122-B1C4-D1AACE76CE9B")) WezTermCommand1 final : public WezTermCommand {
};

CoCreatableClass(WezTermCommand1)
