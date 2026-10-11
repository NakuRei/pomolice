@echo off
msedgedriver.exe --verbose --log-path="%POMOLICE_E2E_RESULTS%\edgedriver-%RANDOM%-%RANDOM%.log" %*
set "driver_exit=%errorlevel%"
echo %date% %time% exit=%driver_exit%>>"%POMOLICE_E2E_RESULTS%\edgedriver-exits.log"
exit /b %driver_exit%
