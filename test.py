import unittest
import requests

BASE_URL = 'http://127.0.0.1:3030/api/'

# Tests assume the following account are created, and will be re-created as-is with a POST to /api/reset:
# {'first_name': 'Admin', 'last_name': 'User', 'username': 'admin', 'password': 'admin'}
# {'first_name': 'Professor', 'last_name': 'User', 'username': 'professor', 'password': 'professor'}
# {'first_name': 'Student', 'last_name': 'User', 'username': 'student', 'password': 'student'}

# Tests are not safe to run in parallel - they might manipulate the same accounts at the same time.


def validate_object(data, schema, _path=''):
    '''Checks that a dict follows a schema. Can check for either type or value equality.'''

    if type(schema) == dict:
        assert type(data) == dict, f'{_path} is not a dict'

        for key, key_schema in schema.items():
            validate_object(data[key], key_schema, _path=_path + f'.{key}')

    elif type(schema) == list:
        assert type(data) == list, f'{_path} is not a list'

        for index, item in enumerate(data):
            validate_object(item, schema[0], _path=_path + f'[{index}]')

    elif type(schema) == type:
        assert type(
            data) == schema, f'{_path} is of type {type(data)}, but should have been {schema}'

    else:
        assert data == schema, f'{_path} equals "{data}", where it should have been "{schema}"'


def get_token(username, password=None):
    res = requests.post(BASE_URL + 'session', json={
        'username': username,
        'password': username if password is None else password,
    })
    assert res.status_code == 200, res.status_code
    return res.json()['token']


def validate_error_response(res, code):
    validate_object(res, {
        'status': 'error',
        'code': code,
    })


def validate_simple_success_response(res):
    validate_object(res, {
        'status': 'success'
    })


def reset():
    res = requests.get(BASE_URL + 'reset')
    assert res.text == '"ok"'


class TestSessionRoutes(unittest.TestCase):

    @classmethod
    def setUpClass(self):
        reset()

    def test_post_session(self):
        user_schema_admin = {
            'first_name': 'Admin',
            'last_name': 'User',
            'kind': 'administrator',
        }

        user_schema_professor = {
            'first_name': 'Professor',
            'last_name': 'User',
            'kind': 'professor',
        }

        user_schema_student = {
            'first_name': 'Student',
            'last_name': 'User',
            'kind': 'student',
        }

        parameters = (
            ('admin', user_schema_admin),
            ('professor', user_schema_professor),
            ('student', user_schema_student)
        )

        for username, user_schema in parameters:
            with self.subTest(msg='Checks that the user can login', username=username):
                res = requests.post(BASE_URL + 'session', json={
                    'username': username,
                    'password': username,
                })
                assert res.status_code == 200, res.status_code

                validate_object(res.json(), {
                    'status': 'success',
                    'token': str,
                    'user': user_schema,
                })

    def test_post_session_invalid_credentials(self):
        res = requests.post(BASE_URL + 'session', json={
            'username': 'not-found',
            'password': 'invalid-password'
        })
        assert res.status_code == 403, res.status_code

        validate_error_response(res.json(), 'InvalidCredentials')

    def test_delete_session(self):
        token = get_token('admin')

        res = requests.delete(BASE_URL + 'session', headers={
            'Authorization': f'Bearer {token}'
        })
        assert res.status_code == 200, res.status_code

        validate_simple_success_response(res.json())

    def test_delete_session_invalid_token(self):
        res = requests.delete(BASE_URL + 'session', headers={
            'Authorization': 'Bearer somerandomfaketoken'
        })
        assert res.status_code == 403, res.status_code

        validate_error_response(res.json(), 'InvalidCredentials')


class TestProfileRoutes(unittest.TestCase):

    def setUp(self):
        reset()

    def test_put_profile_not_authorized(self):
        res = requests.put(BASE_URL + 'profile', headers={
            'Authorization': 'Bearer somerandomfaketoken'
        }, json={})
        assert res.status_code == 403, res.status_code

        validate_error_response(res.json(), 'InvalidCredentials')

    def test_put_profile_first_name_last_name_without_admin(self):
        for prop in ('first_name', 'last_name'):
            for username in ('professor', 'student'):
                with self.subTest(msg='Checks that the property cant be edited', prop=prop, username=username):
                    token = get_token(username)

                    res = requests.put(BASE_URL + 'profile', headers={
                        'Authorization': f'Bearer {token}'
                    }, json={prop: 'new value'})

                    assert res.status_code == 401, res.status_code

                    validate_error_response(
                        res.json(), 'InsufficientAuthorization')

    def test_put_profile_password_without_old(self):
        token = get_token('admin')
        res = requests.put(BASE_URL + 'profile', headers={
            'Authorization': f'Bearer {token}'
        }, json={
            'password': 'newpassword'
        })
        assert res.status_code == 400, res.status_code
        validate_error_response(res.json(), 'MalformedData')

    def test_put_profile_old_password_without_new(self):
        token = get_token('admin')
        res = requests.put(BASE_URL + 'profile', headers={
            'Authorization': f'Bearer {token}'
        }, json={
            'old_password': 'oldpassword'
        })
        assert res.status_code == 400, res.status_code
        validate_error_response(res.json(), 'MalformedData')

    def test_put_profile_wrong_old_password(self):
        token = get_token('admin')
        res = requests.put(BASE_URL + 'profile', headers={
            'Authorization': f'Bearer {token}'
        }, json={
            'old_password': 'wrongpassword',
            'password': 'newpassword',
        })
        assert res.status_code == 403, res.status_code
        validate_error_response(res.json(), 'InvalidOldPassword')

    def test_put_profile_can_change_password(self):
        for account in ('admin', 'professor', 'student'):
            with self.subTest(msg='Checks that the user can change their password', account=account):
                token = get_token(account)
                res = requests.put(BASE_URL + 'profile', headers={
                    'Authorization': f'Bearer {token}'
                }, json={
                    'old_password': account,
                    'password': 'newpassword'
                })
                assert res.status_code == 200, res.status_code
                validate_simple_success_response(res.json())

                res = requests.post(BASE_URL + 'session', json={
                    'username': account,
                    'password': account,
                })
                assert res.status_code == 403, res.status_code
                validate_error_response(res.json(), 'InvalidCredentials')

                get_token(account, 'newpassword')

    def test_put_profile_admin_can_change_first_name_last_name(self):
        token = get_token('admin')
        res = requests.put(BASE_URL + 'profile', headers={
            'Authorization': f'Bearer {token}'
        }, json={
            'first_name': 'NewAdminFirstName',
            'last_name': 'NewAdminLastName',
        })
        assert res.status_code == 200, res.status_code
        validate_simple_success_response(res.json())

        res = requests.post(BASE_URL + 'session', json={
            'username': 'admin',
            'password': 'admin',
        })
        assert res.status_code == 200, res.status_code
        assert res.json()['user']['first_name'] == 'NewAdminFirstName'
        assert res.json()['user']['last_name'] == 'NewAdminLastName'


if __name__ == '__main__':
    unittest.main()
