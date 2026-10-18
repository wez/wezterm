
# Project Setup Guide

## Prerequisites

Before running the project, make sure to download WezTerm and complete the following steps to set up the project correctly.

Additionally, ensure that you have **Visual Studio Build Tools for C++** installed, as they are required to build the project successfully. You can download them from the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) page.

## Step 1: Download WezTerm

1. Download the latest version of WezTerm from the official [WezTerm GitHub repository](https://github.com/wez/wezterm).
2. Extract the files from the WezTerm directory to a location on your computer.

## Step 2: Copy Files to the Project

1. In your project folder, create a folder structure like this:
   ```
   ProjectFolder
   └── WzTerm
       └── WezTerm
   ```
2. Copy all the files from the WezTerm directory (that you downloaded in Step 1) into the `WezTerm` folder inside the `WzTerm` directory in your project.

   After this step, your folder structure should look like this:
   ```
   ProjectFolder
   └── WzTerm
       └── WezTerm
           ├── wezterm.exe
           ├── (other WezTerm files)
   ```

## Step 3: Run the Build Script

1. Once the files are copied into the `WezTerm` folder, navigate to the root of your project folder.
2. Double-click `build.bat` to run the build process.

   The `build.bat` script will now use the files from the WezTerm folder and start the build process.
   
## Notes

The `build.bat` script is designed to automatically run with administrator privileges. This is necessary to install and delete certificates without requiring the user to manually handle these tasks.

When you double-click `build.bat`, it will:
1. Install the required certificate.
2. Perform the build process.
3. Delete the installed certificate automatically.

By running the `build.bat` script with administrator privileges, these tasks can be completed seamlessly, ensuring a smooth build process without any interruptions due to insufficient permissions.

## Troubleshooting

- Ensure that all files from the WezTerm directory are copied to the `WezTerm` folder inside your project.
- If you encounter any issues, check the console for any error messages or missing files.
