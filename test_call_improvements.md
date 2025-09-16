# Test Plan for Call Command Improvements

## Fixed Issues

### Primary Issue: Combined Status Code Error
- **Problem**: User reported `Status: Failed (StatusCode(IS_ERROR | BadUnexpectedError | BadTooManyOperations | BadLicenseNotAvailable))`
- **Root Cause**: Auto-search was overwhelming server with browse operations, causing multiple cascading errors
- **Solution**: Enhanced error handling and conservative search limits

## New Features

### 1. Enhanced Error Handling
- **Specific handling for combined errors**: Recognizes when multiple status codes indicate search overload
- **Detailed troubleshooting guidance**: Provides step-by-step resolution for different error combinations
- **Clear separation**: Distinguishes server-side licensing issues from client-side search problems

### 2. No-Search Option
- **New flag**: `--no-search` to bypass auto-search completely
- **Use case**: For servers with strict browse operation limits
- **Requirement**: Must provide exact node IDs when using this flag

### 3. Conservative Search Limits
- **Deep search limits**: Reduced from 500 to 100 nodes maximum
- **Queue limits**: Reduced from 200 to 50 operations maximum  
- **Gentle timing**: Added 1ms delays to be gentler on sensitive servers
- **Progressive reporting**: Shows progress every 10 nodes instead of 50

## Test Commands

### Test No-Search Flag
```bash
# This should show error requiring exact node IDs
opcua-walker call "Reboot" --no-search

# This should work if node IDs are valid
opcua-walker call "ns=2;i=12345" "ns=2;i=12344" --no-search
```

### Test Enhanced Error Messages
The improved error messages will show:
1. **Multiple error detection**: When BadTooManyOperations + BadLicenseNotAvailable + BadUnexpectedError occur together
2. **Individual error guidance**: Specific help for each error type when they occur alone
3. **Search alternatives**: Guidance to use --no-search flag and exact node IDs

### Test Conservative Search
- Search now caps at 100 nodes total vs previous 500
- Should be much gentler on servers with browse operation limits
- Still maintains cross-namespace discovery capability

## Expected Outcomes

1. **Reduced server overload**: Conservative limits should prevent BadTooManyOperations
2. **Clear user guidance**: Users know exactly how to work around server limitations
3. **Backward compatibility**: All existing functionality preserved, just with better limits
4. **Alternative approach**: --no-search provides escape hatch for problematic servers