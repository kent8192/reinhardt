# E2E Test Scenarios

## Overview
This document defines end-to-end test scenarios for the Twitter clone application. It covers integration tests between the WASM frontend and backend server, simulating actual user flows.

## Test Environment Requirements
- **Frontend**: WASM application (`dist-wasm/` build output)
- **Backend**: Development server (port 8000)
- **Database**: SQLite (with test migrations applied)
- **Test Framework**: Playwright
- **Browser**: Chromium (headless mode recommended)

## Test Data Setup
The following test data is required as a precondition for each test:

```typescript
// Test users
const TEST_USERS = {
  existing: {
    username: 'testuser',
    email: 'test@example.com',
    password: 'Password123!'
  },
  new: {
    username: 'newuser',
    email: 'new@example.com',
    password: 'NewPassword123!'
  }
};

// Test tweets
const TEST_TWEETS = {
  simple: 'This is a test tweet.',
  long: 'x'.repeat(280), // Maximum length
  withHashtag: '#test #TwitterClone'
};
```

## Scenario 1: New User Registration Flow
### Overview
Verifies the flow from a new user registering with the application to being redirected to the timeline page.

### Preconditions
- The application is accessible at `/`
- The test user's email address is not yet registered
- The server is operating normally

### Test Steps
1. **Home page display** -> "Login" and "Register" links are displayed
2. **Navigate to registration page** -> Registration form is displayed (username, email, password, and password confirmation fields)
3. **Enter valid information** -> Enter valid values in all fields
4. **Submit form** -> Success message is displayed, redirect to `/login` page
5. **Verify login page** -> Registration success message and login form are displayed

### Verification Points
- [ ] Required fields in the registration form are properly validated
- [ ] An error message is displayed when passwords do not match
- [ ] An appropriate error message is displayed for duplicate email addresses
- [ ] After successful registration, the user is redirected to the login page
- [ ] A registration success message is displayed on the login page

### Edge Cases
- Insufficient password strength (too short, insufficient complexity)
- Invalid email address format
- Username contains invalid characters
- Behavior when a network error occurs

## Scenario 2: Existing User Login Flow
### Overview
Verifies the flow of an existing user logging in and accessing the timeline page.

### Preconditions
- The test user is already registered
- The user is in a logged-out state

### Test Steps
1. **Login page display** -> Login form is displayed
2. **Enter valid credentials** -> Enter registered email address and password
3. **Submit form** -> Success message is displayed, redirect to `/timeline` page
4. **Verify timeline page** -> Username is displayed, tweet creation form is displayed
5. **Authentication state persistence** -> Login state is maintained after page reload

### Verification Points
- [ ] An appropriate error message is displayed for invalid credentials
- [ ] After successful login, the user is redirected to the timeline page
- [ ] The username is correctly displayed on the timeline page
- [ ] The session is properly maintained
- [ ] A logout link is displayed

### Edge Cases
- Invalid email address format
- Incorrect password
- Login attempt with a non-existent user
- Account has been deactivated

## Scenario 3: Tweet Creation Flow
### Overview
Verifies the flow of a logged-in user creating a new tweet.

### Preconditions
- The user is logged in
- The timeline page (`/timeline`) is accessible

### Test Steps
1. **Timeline page display** -> Tweet creation form is displayed
2. **Enter tweet content** -> Enter valid tweet content (1-280 characters)
3. **Verify character counter** -> Character count updates as input changes
4. **Post tweet** -> Click the post button
5. **Verify successful post** -> Tweet appears on the timeline
6. **Page reload** -> The posted tweet has been persisted

### Verification Points
- [ ] Empty tweets cannot be posted
- [ ] Tweets exceeding 280 characters cannot be posted
- [ ] The character counter works correctly (normal/warning/danger states)
- [ ] After successful posting, the form is cleared
- [ ] The posted tweet appears at the top of the timeline
- [ ] The tweet displays the correct creation timestamp

### Edge Cases
- A tweet with exactly 280 characters
- Tweets containing special characters (emoji, line breaks, hashtags)
- Retry behavior when a network error occurs
- Rate limiting for consecutive posts

## Scenario 4: Tweet Deletion Flow
### Overview
Verifies the flow of a user deleting their own tweet.

### Preconditions
- The user is logged in
- The user has posted at least one tweet
- Tweets are displayed on the timeline page

### Test Steps
1. **Timeline page display** -> Delete button is displayed on the user's tweets
2. **Click delete button** -> Deletion confirmation dialog is displayed (if applicable)
3. **Confirm deletion** -> Execute the deletion
4. **Verify successful deletion** -> Tweet disappears from the timeline
5. **Page reload** -> The deleted tweet does not reappear

### Verification Points
- [ ] Delete button is displayed only on the user's own tweets
- [ ] After deletion, the tweet is immediately hidden
- [ ] The tweet does not reappear after page reload following deletion
- [ ] An appropriate error message is displayed when deletion fails (e.g., insufficient permissions)

### Edge Cases
- Attempting to delete another user's tweet
- Network disconnection during deletion
- Concurrent deletion race condition

## Scenario 5: Profile Display Flow
### Overview
Verifies the flow of a user viewing their own or another user's profile.

### Preconditions
- The user is logged in
- The target user exists
- Profile information (bio, location, website) has been configured

### Test Steps
1. **Navigate to profile page** -> Access `profile/{user_id}`
2. **Verify profile information** -> Username, avatar, bio, location, and website are displayed
3. **Verify follow information** -> Following count and follower count are displayed
4. **Verify tweet list** -> A list of the user's tweets is displayed
5. **Verify actions** -> Follow button and profile edit button are displayed appropriately

### Verification Points
- [ ] A 404 error is displayed for a non-existent user ID
- [ ] An edit button is displayed on the user's own profile
- [ ] A follow button is displayed on other users' profiles
- [ ] Profile information is displayed correctly
- [ ] The user's tweets are correctly filtered and displayed

## Scenario 6: Profile Edit Flow
### Overview
Verifies the flow of a user editing their own profile information.

### Preconditions
- The user is logged in
- The profile edit page (`/profile/{user_id}/edit`) is accessible

### Test Steps
1. **Navigate to profile edit page** -> Edit form is displayed
2. **Verify current values** -> Current profile information is displayed in the form
3. **Update information** -> Update each field
4. **Submit form** -> Click the save button
5. **Verify success** -> Success message is displayed, redirect to profile page
6. **Verify changes** -> Updated information is reflected on the profile page

### Verification Points
- [ ] Current profile information is correctly displayed in the form
- [ ] Validation errors are properly displayed (e.g., invalid URL)
- [ ] After successful save, the user is redirected to the profile page
- [ ] Updated information is correctly reflected
- [ ] The cancel button returns to the profile page

## Scenario 7: Follow/Unfollow Flow
### Overview
Verifies the flow of a user following/unfollowing another user.

### Preconditions
- User A (logged in) and User B (existing user) exist
- User A is not yet following User B

### Test Steps
1. **Display User B's profile** -> Follow button shows "Follow"
2. **Execute follow** -> Click the follow button
3. **Verify state update** -> Button changes to "Following"
4. **Follower count update** -> User B's follower count increases
5. **Execute unfollow** -> Click the "Following" button
6. **Verify state revert** -> Button reverts to "Follow"
7. **Verify follower count decrease** -> User B's follower count decreases

### Verification Points
- [ ] Button text changes correctly based on follow state
- [ ] "Unfollow" is displayed on hover over the follow button
- [ ] Count updates immediately after follow/unfollow
- [ ] State rolls back on network error
- [ ] Users cannot follow themselves

## Scenario 8: Navigation Flow
### Overview
Verifies page navigation flows within the application.

### Preconditions
- The application is operating normally
- The user is logged in

### Test Steps
1. **Home page to login page** -> Click login link
2. **Login page to registration page** -> Click registration link
3. **Registration page to login page** -> Click login link
4. **Timeline page after login** -> Automatic redirect after successful login
5. **Timeline page to profile** -> Click username link
6. **Profile to edit page** -> Click edit button
7. **Edit page to profile** -> Click cancel button
8. **Browser back/forward** -> History navigation works correctly

### Verification Points
- [ ] All navigation links work correctly
- [ ] Pages requiring authentication redirect unauthenticated users
- [ ] Browser back/forward buttons work correctly
- [ ] The current page is correctly highlighted in navigation
- [ ] A 404 page is displayed for non-existent routes

## Scenario 9: Error Handling Flow
### Overview
Verifies the application's error handling capabilities.

### Preconditions
- The application is operating normally

### Test Steps
1. **Network error simulation** -> Perform operations while unable to connect to the server
2. **Invalid form input** -> Trigger validation errors
3. **Permission error** -> Execute an unauthorized operation
4. **Server error** -> Cause a 500 error on the server side
5. **Client error** -> Trigger a JavaScript error

### Verification Points
- [ ] Appropriate error messages are displayed during network errors
- [ ] Form validation errors are displayed near the relevant fields
- [ ] Appropriate messages and alternative actions are provided for permission errors
- [ ] User-friendly messages are displayed during server errors
- [ ] The application continues to function without crashing after errors

## Scenario 10: Responsive Design Flow
### Overview
Verifies layout and functionality across different screen sizes.

### Preconditions
- The application is operating normally
- The user is logged in

### Test Steps
1. **Desktop display** -> Verify display at width 1200px or greater
2. **Tablet display** -> Verify display at width 768px
3. **Mobile display** -> Verify display at width 375px
4. **Screen rotation** -> Verify display in portrait/landscape orientation
5. **Zoom operation** -> Verify display with browser zoom functionality

### Verification Points
- [ ] Layout does not break at any screen width
- [ ] Navigation menu collapses appropriately on mobile
- [ ] Form elements are appropriately sized for touch interaction
- [ ] Images and media scale appropriately based on screen width
- [ ] Text is displayed at a readable size

## Test Implementation Guidelines

### Playwright Configuration
```typescript
// playwright.config.ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:8000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'mobile-chrome',
      use: { ...devices['Pixel 5'] },
    },
  ],
  webServer: {
    command: 'cargo run --bin runserver',
    url: 'http://localhost:8000',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000, // 2 minutes
  },
});
```

### Page Object Pattern
```typescript
// tests/pages/login-page.ts
export class LoginPage {
  constructor(private page: Page) {}

  async navigate() {
    await this.page.goto('/login');
  }

  async login(email: string, password: string) {
    await this.page.fill('input[name="email"]', email);
    await this.page.fill('input[name="password"]', password);
    await this.page.click('button[type="submit"]');
  }

  async getErrorMessage() {
    return this.page.textContent('.alert-danger');
  }

  async isRedirectedToTimeline() {
    return this.page.url().includes('/timeline');
  }
}
```

### Test Data Management
```typescript
// tests/fixtures/test-data.ts
export interface TestUser {
  username: string;
  email: string;
  password: string;
}

export const createTestUser = async (): Promise<TestUser> => {
  // Helper function to create a test user
  // Insert directly into the database or create via API
};

export const cleanupTestData = async (): Promise<void> => {
  // Function to clean up test data
};
```

## Priority and Execution Plan

### High Priority (MVP Features)
1. Scenario 1: New User Registration Flow
2. Scenario 2: Existing User Login Flow
3. Scenario 3: Tweet Creation Flow
4. Scenario 8: Navigation Flow

### Medium Priority (Core Features)
5. Scenario 4: Tweet Deletion Flow
6. Scenario 5: Profile Display Flow
7. Scenario 6: Profile Edit Flow
8. Scenario 7: Follow/Unfollow Flow

### Low Priority (Extended Features)
9. Scenario 9: Error Handling Flow
10. Scenario 10: Responsive Design Flow

## Success Criteria
- All high-priority scenarios pass
- Test coverage covers 80% or more of major user flows
- Test execution time is within 10 minutes
- Flaky tests are less than 5%

---

*Last updated: 2026-01-09*
*Scenario version: 1.0.0*
